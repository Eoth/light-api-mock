use crate::models::{Condition, ConditionGroup, ConditionSource, Operator, Rule};
use std::collections::HashMap;

pub struct MatchEngine;

#[derive(Debug)]
pub struct RequestData {
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub path_params: HashMap<String, String>,
    pub method: String,
    pub remaining_path: String,
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

    fn matches_method(rule_method: &str, request_method: &str) -> bool {
        rule_method.eq_ignore_ascii_case(request_method)
    }

    fn matches_sub_path(
        sub_path: &Option<String>,
        remaining: &str,
    ) -> Option<HashMap<String, String>> {
        match sub_path {
            None => Some(HashMap::new()),
            Some(pattern) => match_path(pattern, remaining).map(|(params, _)| params),
        }
    }

    fn matches_group(group: &ConditionGroup, req: &RequestData) -> bool {
        let all_ok = group.all_of.is_empty() || group.all_of.iter().all(|c| Self::eval(c, req));
        let any_ok = group.any_of.is_empty() || group.any_of.iter().any(|c| Self::eval(c, req));
        all_ok && any_ok
    }

    fn eval(condition: &Condition, req: &RequestData) -> bool {
        let extracted = Self::extract(&condition.source, req);
        Self::apply_op(&condition.operator, extracted.as_deref())
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

pub(crate) fn match_path(
    listen_path: &str,
    request_path: &str,
) -> Option<(HashMap<String, String>, String)> {
    let pattern_str = normalize_colon_syntax(listen_path);

    let pattern_segs: Vec<&str> = pattern_str.split('/').filter(|s| !s.is_empty()).collect();
    let request_segs: Vec<&str> = request_path.split('/').filter(|s| !s.is_empty()).collect();

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
            script: None,
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
    fn match_path_empty_never_matches() {
        assert!(match_path("", "/").is_none());
        assert!(match_path("/", "/").is_none());
    }
}
