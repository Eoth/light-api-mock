use crate::models::{Condition, ConditionGroup, ConditionSource, Operator, Rule};
use serde::Serialize;
use std::collections::HashMap;

pub struct MatchEngine;

#[derive(Debug, Clone)]
pub struct RequestData {
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub path_params: HashMap<String, String>,
    pub method: String,
    pub remaining_path: String,
}

/// Resultat detaille de l'evaluation d'UNE condition — utilise uniquement par le
/// testeur de regle (UI), jamais par le chemin de production (`matches_group`,
/// qui reste un simple booleen pour ne pas alourdir le hot path HTTP). Expose
/// la valeur trouvee (ou son absence) et, si la condition echoue, un indice
/// "cross-source" (`hint`) quand la meme cle existe dans une AUTRE source de
/// la requete (ex. choix de source probablement errone : path param au lieu
/// de query param).
#[derive(Debug, Clone, Serialize)]
pub struct ConditionEvaluation {
    pub condition: Condition,
    pub matched: bool,
    pub found_value: Option<String>,
    pub hint: Option<String>,
}

/// Resultat detaille de l'evaluation d'un `ConditionGroup` (all_of + any_of),
/// miroir instrumente de `MatchEngine::matches_group`. Voir le commentaire sur
/// `evaluate_group` pour le choix de dupliquer la glue all/any plutot que d'y
/// faire deleguer `matches_group`.
#[derive(Debug, Clone, Serialize)]
pub struct GroupEvaluation {
    pub all_of: Vec<ConditionEvaluation>,
    pub any_of: Vec<ConditionEvaluation>,
    pub matched: bool,
}

/// Entree du testeur de regle : le brouillon de regle en cours d'edition
/// (pas necessairement sauvegarde), teste contre une requete deja capturee.
pub struct RuleTestInput<'a> {
    pub method: &'a str,
    pub sub_path: &'a Option<String>,
    pub conditions: &'a ConditionGroup,
}

/// Resultat complet du testeur de regle : method/sub_path/conditions, chacun
/// avec son propre statut, plus le detail par condition.
#[derive(Debug, Clone, Serialize)]
pub struct RuleTestOutcome {
    pub method_matches: bool,
    pub sub_path_matches: bool,
    pub path_params: HashMap<String, String>,
    pub group: GroupEvaluation,
    pub overall_matched: bool,
}

impl MatchEngine {
    pub fn first_match<'a>(
        rules: &'a [Rule],
        req: &RequestData,
    ) -> Option<(&'a Rule, HashMap<String, String>)> {
        rules.iter().find_map(|rule| {
            if !Self::matches_method(&rule.method, &req.method) {
                return None;
            }
            let sub_params = Self::matches_sub_path(&rule.sub_path, &req.remaining_path)?;
            if !Self::matches_group(&rule.conditions, req) {
                return None;
            }
            Some((rule, sub_params))
        })
    }

    /// pub(crate) (plutot que privee) : reutilisee par `evaluate_rule_test`
    /// (testeur de regle) en plus de `first_match` (chemin de production).
    pub(crate) fn matches_method(rule_method: &str, request_method: &str) -> bool {
        rule_method.eq_ignore_ascii_case(request_method)
    }

    /// pub(crate) (plutot que privee) : idem, reutilisee par `evaluate_rule_test`.
    pub(crate) fn matches_sub_path(
        sub_path: &Option<String>,
        remaining: &str,
    ) -> Option<HashMap<String, String>> {
        match sub_path {
            None => Some(HashMap::new()),
            Some(pattern) => match_path(pattern, remaining).map(|(params, _)| params),
        }
    }

    /// pub(crate) (plutot que privee) : reutilisee telle quelle par
    /// `messaging::matcher` (feature "messaging-kafka") pour matcher un
    /// message Kafka contre les conditions d'une regle, sans les notions
    /// HTTP-only method/sub_path (voir commentaire dans messaging/matcher.rs).
    pub(crate) fn matches_group(group: &ConditionGroup, req: &RequestData) -> bool {
        let all_ok = group.all_of.is_empty() || group.all_of.iter().all(|c| Self::eval(c, req));
        let any_ok = group.any_of.is_empty() || group.any_of.iter().any(|c| Self::eval(c, req));
        all_ok && any_ok
    }

    fn eval(condition: &Condition, req: &RequestData) -> bool {
        let extracted = Self::extract(&condition.source, req);
        Self::apply_op(&condition.operator, extracted.as_deref())
    }

    /// Variante instrumentee de `matches_group`, pour le testeur de regle
    /// (UI) uniquement. Reutilise les MEMES primitives que le chemin de
    /// production (`extract`/`apply_op`) : aucune logique de matching n'est
    /// dupliquee, seule la glue "all/any" ci-dessous l'est (~5 lignes).
    ///
    /// Pourquoi ne pas faire deleguer `matches_group` a cette fonction : le
    /// chemin HTTP de production appelle `matches_group` sur CHAQUE requete
    /// recue (potentiellement un fort volume) ; construire ici le detail
    /// complet (clone de `Condition`, calcul de hints, allocations de String)
    /// pour un resultat immediatement jete ailleurs que dans ce testeur
    /// alourdirait ce hot path sans aucun benefice. `matches_group` reste
    /// donc un booleen pur, et un test dedie (`evaluate_group_matches_agree_with_matches_group`,
    /// voir tests) garantit que les deux ne divergent jamais, plutot que de
    /// s'appuyer sur une delegation qui masquerait ce cout de perf.
    pub fn evaluate_group(group: &ConditionGroup, req: &RequestData) -> GroupEvaluation {
        let all_of: Vec<ConditionEvaluation> =
            group.all_of.iter().map(|c| Self::eval_detailed(c, req)).collect();
        let any_of: Vec<ConditionEvaluation> =
            group.any_of.iter().map(|c| Self::eval_detailed(c, req)).collect();
        let all_ok = all_of.is_empty() || all_of.iter().all(|e| e.matched);
        let any_ok = any_of.is_empty() || any_of.iter().any(|e| e.matched);
        GroupEvaluation {
            all_of,
            any_of,
            matched: all_ok && any_ok,
        }
    }

    fn eval_detailed(condition: &Condition, req: &RequestData) -> ConditionEvaluation {
        let found_value = Self::extract(&condition.source, req);
        let matched = Self::apply_op(&condition.operator, found_value.as_deref());
        let hint = if matched {
            None
        } else {
            Self::cross_source_hint(&condition.source, req)
        };
        ConditionEvaluation {
            condition: condition.clone(),
            matched,
            found_value,
            hint,
        }
    }

    /// Suggestion pedagogique quand une condition a cle simple (QueryParam,
    /// Header, PathParam, FormField) ne matche pas : cherche si la MEME cle
    /// existe dans une AUTRE source de la requete capturee, pour signaler un
    /// choix de source probablement errone (ex. "je voulais PathParam, j'ai
    /// mis QueryParam"). Pas de hint pour JsonPointer/XPath/BodyRaw : ce ne
    /// sont pas de simples cles nommees comparables entre elles (chemin
    /// structure ou corps entier, pas un nom de champ).
    fn cross_source_hint(source: &ConditionSource, req: &RequestData) -> Option<String> {
        let (key, current_label) = match source {
            ConditionSource::QueryParam(k) => (k, "parametre de requete"),
            ConditionSource::Header(k) => (k, "en-tete"),
            ConditionSource::PathParam(k) => (k, "parametre de chemin"),
            ConditionSource::FormField(k) => (k, "champ de formulaire"),
            _ => return None,
        };

        let mut found_in = Vec::new();
        if !matches!(source, ConditionSource::PathParam(_)) && req.path_params.contains_key(key) {
            found_in.push("parametre de chemin");
        }
        if !matches!(source, ConditionSource::QueryParam(_)) && req.query_params.contains_key(key)
        {
            found_in.push("parametre de requete");
        }
        if !matches!(source, ConditionSource::Header(_))
            && req.headers.keys().any(|h| h.eq_ignore_ascii_case(key))
        {
            found_in.push("en-tete");
        }

        if found_in.is_empty() {
            None
        } else {
            Some(format!(
                "'{key}' n'a pas ete trouve comme {current_label}, mais est present comme {} dans cette requete",
                found_in.join(" et ")
            ))
        }
    }

    /// Point d'entree unique du testeur de regle (endpoint `POST /api/rule-test`) :
    /// recalcule method/sub_path/conditions pour le brouillon de regle en
    /// cours d'edition contre une requete deja capturee (`RequestLog`), sans
    /// aucune mutation ni appel reseau. Reutilise `matches_method`/
    /// `matches_sub_path`/`evaluate_group` — le meme trio que `first_match`,
    /// zero logique de matching dupliquee.
    pub fn evaluate_rule_test(input: RuleTestInput, req: &RequestData) -> RuleTestOutcome {
        let method_matches = Self::matches_method(input.method, &req.method);
        let sub_params = Self::matches_sub_path(input.sub_path, &req.remaining_path);
        let sub_path_matches = sub_params.is_some();

        let mut path_params = req.path_params.clone();
        if let Some(p) = sub_params {
            path_params.extend(p);
        }

        let merged_req = RequestData {
            path_params: path_params.clone(),
            ..req.clone()
        };
        let group = Self::evaluate_group(input.conditions, &merged_req);

        RuleTestOutcome {
            method_matches,
            sub_path_matches,
            overall_matched: method_matches && sub_path_matches && group.matched,
            path_params,
            group,
        }
    }

    fn extract(source: &ConditionSource, req: &RequestData) -> Option<String> {
        match source {
            ConditionSource::QueryParam(key) => req.query_params.get(key).cloned(),
            ConditionSource::Header(key) => {
                let lower = key.to_ascii_lowercase();
                req.headers
                    .iter()
                    .find(|(k, _)| k.to_ascii_lowercase() == lower)
                    .map(|(_, v)| v.clone())
            }
            ConditionSource::JsonPointer(pointer) => Self::extract_json_pointer(&req.body, pointer),
            ConditionSource::XPath(path) => Self::extract_xpath(&req.body, path),
            ConditionSource::FormField(field) => Self::extract_form_field(&req.body, field),
            ConditionSource::PathParam(key) => req.path_params.get(key).cloned(),
            ConditionSource::BodyRaw => String::from_utf8(req.body.clone()).ok(),
        }
    }

    fn extract_json_pointer(body: &[u8], pointer: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_slice(body).ok()?;
        let found = value.pointer(pointer)?;
        match found {
            serde_json::Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        }
    }

    fn extract_xpath(body: &[u8], path: &str) -> Option<String> {
        let text = std::str::from_utf8(body).ok()?;
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        Self::walk_xml(text, &segments)
    }

    fn walk_xml(xml: &str, segments: &[&str]) -> Option<String> {
        use quick_xml::events::Event;
        use quick_xml::reader::Reader;

        let mut reader = Reader::from_str(xml);
        let mut depth_match = 0usize;
        let mut capture = false;
        let mut result = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let local = Self::local_name(&e);
                    if depth_match < segments.len() && local == segments[depth_match] {
                        depth_match += 1;
                        if depth_match == segments.len() {
                            capture = true;
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    if capture {
                        if let Ok(t) = e.unescape() {
                            result.push_str(&t);
                        }
                    }
                }
                Ok(Event::End(_)) => {
                    if capture {
                        return Some(result);
                    }
                    if depth_match > 0 {
                        depth_match -= 1;
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }
        None
    }

    fn local_name(e: &quick_xml::events::BytesStart<'_>) -> String {
        let full = String::from_utf8_lossy(e.name().as_ref()).to_string();
        full.split(':').last().unwrap_or(&full).to_string()
    }

    fn extract_form_field(body: &[u8], field: &str) -> Option<String> {
        let text = std::str::from_utf8(body).ok()?;
        url::form_urlencoded::parse(text.as_bytes())
            .find(|(k, _)| k == field)
            .map(|(_, v)| v.into_owned())
    }

    fn apply_op(op: &Operator, value: Option<&str>) -> bool {
        match op {
            Operator::Exists => value.is_some(),
            Operator::Eq(expected) => value == Some(expected.as_str()),
            Operator::Contains(sub) => value.is_some_and(|v| v.contains(sub.as_str())),
            Operator::Regex(pattern) => {
                let Some(v) = value else { return false };
                regex::Regex::new(pattern).is_ok_and(|re| re.is_match(v))
            }
        }
    }
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

pub(crate) fn match_path(
    listen_path: &str,
    request_path: &str,
) -> Option<(HashMap<String, String>, String)> {
    let pattern_str = normalize_colon_syntax(listen_path);

    let decoded_pat: Vec<String> = pattern_str.split('/').filter(|s| !s.is_empty()).map(|s| url_decode(s)).collect();
    let pattern_segs: Vec<&str> = decoded_pat.iter().map(|s| s.as_str()).collect();
    let decoded_req: Vec<String> = request_path.split('/').filter(|s| !s.is_empty()).map(|s| url_decode(s)).collect();
    let request_segs: Vec<&str> = decoded_req.iter().map(|s| s.as_str()).collect();

    if pattern_segs.is_empty() {
        return None;
    }

    let mut params = HashMap::new();
    let mut has_wildcard = false;
    let mut matched_count = 0;

    for (i, pat) in pattern_segs.iter().enumerate() {
        if *pat == "*" {
            has_wildcard = true;
            matched_count = i;
            break;
        }
        if i >= request_segs.len() {
            return None;
        }
        if pat.starts_with('{') && pat.ends_with('}') {
            let name = &pat[1..pat.len() - 1];
            params.insert(name.to_string(), request_segs[i].to_string());
        } else if *pat != request_segs[i] {
            return None;
        }
        matched_count = i + 1;
    }

    if !has_wildcard && request_segs.len() != pattern_segs.len() {
        return None;
    }

    let remaining = if matched_count < request_segs.len() {
        format!("/{}", request_segs[matched_count..].join("/"))
    } else {
        String::new()
    };

    Some((params, remaining))
}

fn normalize_colon_syntax(s: &str) -> String {
    s.split('/')
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':') {
                format!("{{{name}}}")
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn make_req(
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
        body: &[u8],
        ct: Option<&str>,
    ) -> RequestData {
        RequestData {
            query_params: query
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.to_vec(),
            content_type: ct.map(String::from),
            path_params: HashMap::new(),
            method: "GET".into(),
            remaining_path: String::new(),
        }
    }

    fn simple_rule(name: &str, conditions: ConditionGroup) -> Rule {
        Rule {
            name: name.into(),
            method: "GET".into(),
            sub_path: None,
            action: RuleAction::default(),
            pre_script: None,
            script: None,
            post_script: None,
            conditions,
            response: MockResponse {
                status: 200,
                headers: vec![],
                body: vec![BodyFragment::Literal {
                    value: name.into(),
                }],
                chaos: None,
            },
        }
    }

    #[test]
    fn first_match_returns_first_matching_rule() {
        let rules = vec![
            simple_rule(
                "r1",
                ConditionGroup {
                    all_of: vec![Condition {
                        source: ConditionSource::Header("x-env".into()),
                        operator: Operator::Eq("prod".into()),
                    }],
                    any_of: vec![],
                },
            ),
            simple_rule(
                "r2",
                ConditionGroup {
                    all_of: vec![Condition {
                        source: ConditionSource::Header("x-env".into()),
                        operator: Operator::Eq("staging".into()),
                    }],
                    any_of: vec![],
                },
            ),
        ];
        let req = make_req(&[], &[("x-env", "staging")], b"", None);
        let (matched, _) = MatchEngine::first_match(&rules, &req).unwrap();
        assert_eq!(matched.name, "r2");
    }

    #[test]
    fn no_match_returns_none() {
        let rules = vec![simple_rule(
            "r1",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("x-env".into()),
                    operator: Operator::Eq("prod".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[("x-env", "dev")], b"", None);
        assert!(MatchEngine::first_match(&rules, &req).is_none());
    }

    #[test]
    fn empty_conditions_always_match() {
        let rules = vec![simple_rule("catch-all", ConditionGroup::default())];
        let req = make_req(&[], &[], b"", None);
        let (matched, _) = MatchEngine::first_match(&rules, &req).unwrap();
        assert_eq!(matched.name, "catch-all");
    }

    #[test]
    fn query_param_eq() {
        let rules = vec![simple_rule(
            "qp",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::QueryParam("id".into()),
                    operator: Operator::Eq("42".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[("id", "42")], &[], b"", None);
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn header_case_insensitive() {
        let rules = vec![simple_rule(
            "hdr",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("Content-Type".into()),
                    operator: Operator::Contains("json".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[("content-type", "application/json")], b"", None);
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn json_pointer_extraction() {
        let body = br#"{"user":{"role":"admin","id":5}}"#;
        let rules = vec![simple_rule(
            "jp",
            ConditionGroup {
                all_of: vec![
                    Condition {
                        source: ConditionSource::JsonPointer("/user/role".into()),
                        operator: Operator::Eq("admin".into()),
                    },
                    Condition {
                        source: ConditionSource::JsonPointer("/user/id".into()),
                        operator: Operator::Eq("5".into()),
                    },
                ],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[], body, Some("application/json"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn xpath_extraction() {
        let body = br#"<Envelope><Body><id>123</id></Body></Envelope>"#;
        let rules = vec![simple_rule(
            "xp",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::XPath("Envelope/Body/id".into()),
                    operator: Operator::Eq("123".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[], body, Some("text/xml"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn xpath_with_namespace() {
        let body = br#"<soap:Envelope><soap:Body><ns:id>abc</ns:id></soap:Body></soap:Envelope>"#;
        let rules = vec![simple_rule(
            "xpns",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::XPath("Envelope/Body/id".into()),
                    operator: Operator::Eq("abc".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[], body, Some("text/xml"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn form_field_extraction() {
        let body = b"username=admin&password=secret";
        let rules = vec![simple_rule(
            "form",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::FormField("username".into()),
                    operator: Operator::Eq("admin".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[], body, Some("application/x-www-form-urlencoded"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn body_raw_contains() {
        let body = b"Hello World test payload";
        let rules = vec![simple_rule(
            "raw",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::BodyRaw,
                    operator: Operator::Contains("World".into()),
                }],
                any_of: vec![],
            },
        )];
        let req = make_req(&[], &[], body, None);
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn regex_operator() {
        let rules = vec![simple_rule(
            "rgx",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::QueryParam("code".into()),
                    operator: Operator::Regex(r"^\d{3}$".into()),
                }],
                any_of: vec![],
            },
        )];
        let yes = make_req(&[("code", "200")], &[], b"", None);
        let no = make_req(&[("code", "abcd")], &[], b"", None);
        assert!(MatchEngine::first_match(&rules, &yes).is_some());
        assert!(MatchEngine::first_match(&rules, &no).is_none());
    }

    #[test]
    fn exists_operator() {
        let rules = vec![simple_rule(
            "ex",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("x-debug".into()),
                    operator: Operator::Exists,
                }],
                any_of: vec![],
            },
        )];
        let yes = make_req(&[], &[("x-debug", "")], b"", None);
        let no = make_req(&[], &[], b"", None);
        assert!(MatchEngine::first_match(&rules, &yes).is_some());
        assert!(MatchEngine::first_match(&rules, &no).is_none());
    }

    #[test]
    fn all_of_and_any_of_combined() {
        let rules = vec![simple_rule(
            "combo",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("x-env".into()),
                    operator: Operator::Eq("staging".into()),
                }],
                any_of: vec![
                    Condition {
                        source: ConditionSource::QueryParam("debug".into()),
                        operator: Operator::Exists,
                    },
                    Condition {
                        source: ConditionSource::QueryParam("trace".into()),
                        operator: Operator::Exists,
                    },
                ],
            },
        )];

        let ok1 = make_req(&[("debug", "1")], &[("x-env", "staging")], b"", None);
        let ok2 = make_req(&[("trace", "1")], &[("x-env", "staging")], b"", None);
        let fail_header = make_req(&[("debug", "1")], &[("x-env", "prod")], b"", None);
        let fail_any = make_req(&[], &[("x-env", "staging")], b"", None);

        assert!(MatchEngine::first_match(&rules, &ok1).is_some());
        assert!(MatchEngine::first_match(&rules, &ok2).is_some());
        assert!(MatchEngine::first_match(&rules, &fail_header).is_none());
        assert!(MatchEngine::first_match(&rules, &fail_any).is_none());
    }

    // --- Method matching tests ---

    #[test]
    fn rule_method_get_matches_get_only() {
        let mut rule = simple_rule("get-only", ConditionGroup::default());
        rule.method = "GET".into();
        let rules = vec![rule];

        let mut req_get = make_req(&[], &[], b"", None);
        req_get.method = "GET".into();
        assert!(MatchEngine::first_match(&rules, &req_get).is_some());

        let mut req_post = make_req(&[], &[], b"", None);
        req_post.method = "POST".into();
        assert!(MatchEngine::first_match(&rules, &req_post).is_none());
    }

    #[test]
    fn rule_method_must_match_exactly() {
        let mut rule = simple_rule("get-only", ConditionGroup::default());
        rule.method = "GET".into();
        let rules = vec![rule];

        let mut req_post = make_req(&[], &[], b"", None);
        req_post.method = "POST".into();
        assert!(MatchEngine::first_match(&rules, &req_post).is_none());
    }

    #[test]
    fn sub_path_matches_remaining() {
        let mut rule = simple_rule("sub", ConditionGroup::default());
        rule.sub_path = Some("/users/{id}".into());
        let rules = vec![rule];

        let mut req = make_req(&[], &[], b"", None);
        req.remaining_path = "/users/42".into();
        let (matched, sub_params) = MatchEngine::first_match(&rules, &req).unwrap();
        assert_eq!(matched.name, "sub");
        assert_eq!(sub_params.get("id").unwrap(), "42");
    }

    #[test]
    fn sub_path_no_match() {
        let mut rule = simple_rule("sub", ConditionGroup::default());
        rule.sub_path = Some("/users/{id}".into());
        let rules = vec![rule];

        let mut req = make_req(&[], &[], b"", None);
        req.remaining_path = "/orders/1".into();
        assert!(MatchEngine::first_match(&rules, &req).is_none());
    }

    #[test]
    fn method_and_sub_path_combined() {
        let mut get_rule = simple_rule("get-users", ConditionGroup::default());
        get_rule.method = "GET".into();
        get_rule.sub_path = Some("/users/{id}".into());

        let mut post_rule = simple_rule("post-users", ConditionGroup::default());
        post_rule.method = "POST".into();
        post_rule.sub_path = Some("/users".into());

        let rules = vec![get_rule, post_rule];

        let mut req_get = make_req(&[], &[], b"", None);
        req_get.method = "GET".into();
        req_get.remaining_path = "/users/42".into();
        let (matched, params) = MatchEngine::first_match(&rules, &req_get).unwrap();
        assert_eq!(matched.name, "get-users");
        assert_eq!(params.get("id").unwrap(), "42");

        let mut req_post = make_req(&[], &[], b"", None);
        req_post.method = "POST".into();
        req_post.remaining_path = "/users".into();
        let (matched, _) = MatchEngine::first_match(&rules, &req_post).unwrap();
        assert_eq!(matched.name, "post-users");
    }

    // --- match_path tests ---

    #[test]
    fn match_path_wildcard() {
        let r = match_path("/svc-a/*", "/svc-a/foo/bar");
        assert!(r.is_some());
        let (_, remaining) = r.unwrap();
        assert_eq!(remaining, "/foo/bar");
    }

    #[test]
    fn match_path_named_param() {
        let r = match_path("/v4/insee/{siret}", "/v4/insee/44306184100047");
        assert!(r.is_some());
        let (params, _) = r.unwrap();
        assert_eq!(params.get("siret").unwrap(), "44306184100047");
    }

    #[test]
    fn match_path_url_encoded() {
        let r = match_path("/api/v1/*", "/api/v1/hello%20world");
        assert!(r.is_some());
        let (_, remaining) = r.unwrap();
        assert_eq!(remaining, "/hello world");
    }

    #[test]
    fn match_path_encoded_param() {
        let r = match_path("/users/{name}", "/users/John%20Doe");
        assert!(r.is_some());
        let (params, _) = r.unwrap();
        assert_eq!(params.get("name").unwrap(), "John Doe");
    }

    #[test]
    fn match_path_special_chars() {
        let r = match_path("/api/*", "/api/path%3Awith%3Acolons");
        assert!(r.is_some());
        let (_, remaining) = r.unwrap();
        assert_eq!(remaining, "/path:with:colons");
    }

    #[test]
    fn match_path_jenkins_spaces_in_pattern() {
        let pattern = "/job/Zone - Services/job/{branch}/build";
        let url = "/job/Zone%20-%20Services/job/develop/build";
        let r = match_path(pattern, url);
        assert!(r.is_some(), "spaces in pattern must match %20 in URL");
        let (params, _) = r.unwrap();
        assert_eq!(params.get("branch").unwrap(), "develop");
    }

    #[test]
    fn match_path_pattern_encoded() {
        let pattern = "/job/Zone%20-%20Services/job/{branch}/build";
        let url = "/job/Zone%20-%20Services/job/main/build";
        let r = match_path(pattern, url);
        assert!(r.is_some(), "%20 in pattern must match %20 in URL");
        let (params, _) = r.unwrap();
        assert_eq!(params.get("branch").unwrap(), "main");
    }

    #[test]
    fn match_path_jenkins_deep_nested() {
        let pattern = "/job/Zone - Services aux usagers/job/QUARTIER-Gestion/job/ILOT-Cnt/job/{space}/job/{project}/build";
        let url = "/job/Zone%20-%20Services%20aux%20usagers/job/QUARTIER-Gestion/job/ILOT-Cnt/job/my-space/job/my-project/build";
        let r = match_path(pattern, url);
        assert!(r.is_some());
        let (params, _) = r.unwrap();
        assert_eq!(params.get("space").unwrap(), "my-space");
        assert_eq!(params.get("project").unwrap(), "my-project");
    }

    #[test]
    fn match_path_empty_never_matches() {
        assert!(match_path("", "/").is_none());
        assert!(match_path("/", "/").is_none());
    }

    // --- evaluate_group / ConditionEvaluation tests (testeur de regle) ---

    fn cg(all_of: Vec<Condition>, any_of: Vec<Condition>) -> ConditionGroup {
        ConditionGroup { all_of, any_of }
    }

    #[test]
    fn evaluate_group_reports_found_value_on_match() {
        let group = cg(
            vec![Condition {
                source: ConditionSource::QueryParam("id".into()),
                operator: Operator::Eq("42".into()),
            }],
            vec![],
        );
        let req = make_req(&[("id", "42")], &[], b"", None);
        let result = MatchEngine::evaluate_group(&group, &req);
        assert!(result.matched);
        assert!(result.all_of[0].matched);
        assert_eq!(result.all_of[0].found_value.as_deref(), Some("42"));
        assert!(result.all_of[0].hint.is_none());
    }

    #[test]
    fn evaluate_group_hint_query_param_found_as_path_param() {
        // Cas exact du sujet : l'utilisateur voulait un path param mais a
        // choisi query param par erreur.
        let group = cg(
            vec![Condition {
                source: ConditionSource::QueryParam("foo".into()),
                operator: Operator::Eq("bar".into()),
            }],
            vec![],
        );
        let mut req = make_req(&[], &[], b"", None);
        req.path_params.insert("foo".into(), "bar".into());
        let result = MatchEngine::evaluate_group(&group, &req);
        assert!(!result.matched);
        assert!(!result.all_of[0].matched);
        assert!(result.all_of[0].found_value.is_none());
        let hint = result.all_of[0].hint.as_deref().unwrap();
        assert!(hint.contains("foo"));
        assert!(hint.contains("parametre de chemin"));
    }

    #[test]
    fn evaluate_group_hint_path_param_found_as_header() {
        let group = cg(
            vec![Condition {
                source: ConditionSource::PathParam("token".into()),
                operator: Operator::Exists,
            }],
            vec![],
        );
        let req = make_req(&[], &[("token", "abc")], b"", None);
        let result = MatchEngine::evaluate_group(&group, &req);
        assert!(!result.all_of[0].matched);
        assert_eq!(
            result.all_of[0].hint.as_deref(),
            Some(
                "'token' n'a pas ete trouve comme parametre de chemin, mais est present comme en-tete dans cette requete"
            )
        );
    }

    #[test]
    fn evaluate_group_no_hint_when_key_nowhere_else() {
        let group = cg(
            vec![Condition {
                source: ConditionSource::QueryParam("missing".into()),
                operator: Operator::Exists,
            }],
            vec![],
        );
        let req = make_req(&[], &[], b"", None);
        let result = MatchEngine::evaluate_group(&group, &req);
        assert!(!result.all_of[0].matched);
        assert!(result.all_of[0].hint.is_none());
    }

    #[test]
    fn evaluate_group_no_hint_for_structured_sources() {
        // JsonPointer/XPath/BodyRaw ne sont pas des cles nommees comparables :
        // jamais de hint cross-source, meme en echec.
        let group = cg(
            vec![Condition {
                source: ConditionSource::JsonPointer("/missing".into()),
                operator: Operator::Exists,
            }],
            vec![],
        );
        let req = make_req(&[], &[], b"{}", Some("application/json"));
        let result = MatchEngine::evaluate_group(&group, &req);
        assert!(!result.all_of[0].matched);
        assert!(result.all_of[0].hint.is_none());
    }

    #[test]
    fn evaluate_group_matches_agree_with_matches_group() {
        // Garde de non-regression : evaluate_group ne doit JAMAIS diverger du
        // resultat booleen du chemin de production, sur toute une matrice de
        // cas (match/no-match, all_of/any_of, differentes sources).
        let cases: Vec<(ConditionGroup, RequestData)> = vec![
            (
                cg(
                    vec![Condition {
                        source: ConditionSource::Header("x-env".into()),
                        operator: Operator::Eq("prod".into()),
                    }],
                    vec![],
                ),
                make_req(&[], &[("x-env", "prod")], b"", None),
            ),
            (
                cg(
                    vec![Condition {
                        source: ConditionSource::Header("x-env".into()),
                        operator: Operator::Eq("prod".into()),
                    }],
                    vec![],
                ),
                make_req(&[], &[("x-env", "dev")], b"", None),
            ),
            (
                cg(
                    vec![],
                    vec![
                        Condition {
                            source: ConditionSource::QueryParam("debug".into()),
                            operator: Operator::Exists,
                        },
                        Condition {
                            source: ConditionSource::QueryParam("trace".into()),
                            operator: Operator::Exists,
                        },
                    ],
                ),
                make_req(&[("trace", "1")], &[], b"", None),
            ),
            (ConditionGroup::default(), make_req(&[], &[], b"", None)),
        ];

        for (group, req) in cases {
            let boolean_result = MatchEngine::matches_group(&group, &req);
            let detailed_result = MatchEngine::evaluate_group(&group, &req).matched;
            assert_eq!(
                boolean_result, detailed_result,
                "matches_group et evaluate_group doivent toujours s'accorder"
            );
        }
    }

    // --- evaluate_rule_test tests ---

    #[test]
    fn evaluate_rule_test_nominal_match() {
        let conditions = cg(
            vec![Condition {
                source: ConditionSource::PathParam("id".into()),
                operator: Operator::Eq("42".into()),
            }],
            vec![],
        );
        let sub_path = Some("/orders/{id}".into());
        let mut req = make_req(&[], &[], b"", None);
        req.method = "GET".into();
        req.remaining_path = "/orders/42".into();

        let outcome = MatchEngine::evaluate_rule_test(
            RuleTestInput {
                method: "GET",
                sub_path: &sub_path,
                conditions: &conditions,
            },
            &req,
        );

        assert!(outcome.method_matches);
        assert!(outcome.sub_path_matches);
        assert!(outcome.overall_matched);
        assert_eq!(outcome.path_params.get("id").unwrap(), "42");
        assert!(outcome.group.matched);
    }

    #[test]
    fn evaluate_rule_test_method_mismatch() {
        let conditions = ConditionGroup::default();
        let sub_path = None;
        let mut req = make_req(&[], &[], b"", None);
        req.method = "POST".into();

        let outcome = MatchEngine::evaluate_rule_test(
            RuleTestInput {
                method: "GET",
                sub_path: &sub_path,
                conditions: &conditions,
            },
            &req,
        );

        assert!(!outcome.method_matches);
        assert!(!outcome.overall_matched);
    }

    #[test]
    fn evaluate_rule_test_sub_path_mismatch_still_reports_conditions() {
        let conditions = cg(
            vec![Condition {
                source: ConditionSource::Header("x-env".into()),
                operator: Operator::Eq("prod".into()),
            }],
            vec![],
        );
        let sub_path = Some("/orders/{id}".into());
        let mut req = make_req(&[], &[("x-env", "prod")], b"", None);
        req.remaining_path = "/other/path".into();

        let outcome = MatchEngine::evaluate_rule_test(
            RuleTestInput {
                method: "GET",
                sub_path: &sub_path,
                conditions: &conditions,
            },
            &req,
        );

        assert!(!outcome.sub_path_matches);
        assert!(!outcome.overall_matched);
        // Le detail des conditions reste calcule et exact meme si le
        // sub_path lui-meme ne matche pas — utile pour le diagnostic UI.
        assert!(outcome.group.matched);
    }

    #[test]
    fn evaluate_rule_test_no_sub_path_matches_any_remaining() {
        let conditions = ConditionGroup::default();
        let sub_path = None;
        let mut req = make_req(&[], &[], b"", None);
        req.remaining_path = "/anything/at/all".into();

        let outcome = MatchEngine::evaluate_rule_test(
            RuleTestInput {
                method: "GET",
                sub_path: &sub_path,
                conditions: &conditions,
            },
            &req,
        );

        assert!(outcome.sub_path_matches);
        assert!(outcome.path_params.is_empty());
    }
}
