// Calcul de suggestions de regles de mock a partir des echanges captures par
// `server::observation` pendant qu'un service est explicitement observe.
// TOUJOURS recalcule a la demande depuis `ObservationStore` (jamais de
// cache/etat de suggestion mis a jour separement) : le volume est deja borne
// par construction (buffer d'observation lui-meme borne, cf observation.rs),
// donc recalculer a chaque lecture est moins cher et plus sur qu'un cache a
// invalider — pas de decalage possible entre "ce qui a ete observe" et "ce
// qui est propose".
//
// Piege central (souleve explicitement en amont de ce chantier, a ne jamais
// perdre de vue) : deux appels au meme (service, method, sub_path) peuvent
// legitimement renvoyer des reponses differentes. Une regle inconditionnelle
// batie sur la premiere reponse observee casserait silencieusement les autres
// cas. D'ou l'algorithme en 3 temps : (1) partitionner les observations par
// reponse EQUIVALENTE, (2) si une seule classe -> regle inconditionnelle,
// (3) si plusieurs classes -> chercher un champ de la requete qui les
// discrimine PARFAITEMENT (meme valeur => meme classe, classes differentes
// => valeurs differentes) avant de proposer quoi que ce soit ; si aucun champ
// ne discrimine parfaitement, ne RIEN proposer (silence explicite plutot
// qu'une regle fragile).
use crate::models::{Condition, ConditionSource, HeaderEntry, MockResponse, Operator};
use crate::server::observation::ObservedExchange;
use std::collections::{BTreeSet, HashMap};

const DEFAULT_MIN_SAMPLES: usize = 3;

/// Nombre minimum d'observations d'une cle (service, method, sub_path) avant
/// de tenter quoi que ce soit. En-dessous, une seule reponse observee (ou
/// deux identiques) ne prouve rien sur la stabilite reelle de l'endpoint.
pub fn min_samples() -> usize {
    std::env::var("TRAFFIC_OBSERVATION_MIN_SAMPLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MIN_SAMPLES)
}

/// En-tetes jamais retenues comme champ discriminant : varient d'un appel a
/// l'autre par nature (horodatage, identifiants de correlation/traçage,
/// authentification) sans rapport avec une DECISION metier de la cible.
/// Les retenir produirait une condition techniquement "parfaite" sur
/// l'echantillon mais inutilisable/dangereuse une fois generalisee (ex. une
/// regle conditionnee sur un jeton d'auth ne matchera plus jamais).
const NOISY_HEADERS: &[&str] = &[
    "date",
    "x-request-id",
    "x-correlation-id",
    "traceparent",
    "tracestate",
    "authorization",
    "cookie",
    "set-cookie",
    "user-agent",
];

/// Une regle proposee : soit LA regle inconditionnelle (aucune variance
/// observee), soit UNE des N regles conditionnelles couvrant chaque classe de
/// reponse distincte observee (`condition` alors `Some`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SuggestedRule {
    pub method: String,
    pub sub_path: String,
    pub condition: Option<Condition>,
    pub sample_count: usize,
    pub response: MockResponse,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "outcome")]
pub enum Suggestion {
    /// Aucune variance sur l'echantillon : une seule regle, sans condition.
    Unconditional { rule: Box<SuggestedRule> },
    /// Variance expliquee par un champ de la requete : une regle par valeur
    /// distincte observee, chacune avec sa condition `Eq`.
    Conditional { rules: Vec<SuggestedRule> },
    /// Variance observee mais aucun champ ne l'explique de facon fiable sur
    /// l'echantillon : rien a proposer, seulement un signal diagnostique.
    VarianceUnexplained {
        sample_count: usize,
        response_class_count: usize,
    },
}

/// Signature d'egalite de reponse : statut + corps octet-a-octet (le corps
/// retenu est deja tronque a `observation::max_body_size()`, cf limitation
/// documentee la-bas — deux corps distincts au-dela de la troncature qui
/// partagent le meme prefixe seraient vus ici comme "la meme reponse").
/// Les en-tetes de reponse ne participent PAS a la signature : deux appels
/// avec un corps identique mais un en-tete de correlation different ne
/// doivent pas etre vus comme deux reponses "differentes" a expliquer.
fn response_signature(exchange: &ObservedExchange) -> (u16, &str) {
    (exchange.response_status, exchange.response_body.as_str())
}

/// Calcule la suggestion pour UNE cle (service, method, sub_path) a partir de
/// ses observations retenues. `None` si l'echantillon est encore trop petit
/// (`min_samples()`) — pas encore de decision a prendre, ni positive ni
/// negative.
pub fn suggest(
    method: &str,
    sub_path: &str,
    observations: &[ObservedExchange],
) -> Option<Suggestion> {
    if observations.len() < min_samples() {
        return None;
    }

    let mut classes: Vec<Vec<&ObservedExchange>> = Vec::new();
    for obs in observations {
        let sig = response_signature(obs);
        match classes.iter_mut().find(|c| response_signature(c[0]) == sig) {
            Some(class) => class.push(obs),
            None => classes.push(vec![obs]),
        }
    }

    if classes.len() == 1 {
        let representative = classes[0].last().expect("classe non vide");
        return Some(Suggestion::Unconditional {
            rule: Box::new(SuggestedRule {
                method: method.to_string(),
                sub_path: sub_path.to_string(),
                condition: None,
                sample_count: observations.len(),
                response: build_response(representative),
            }),
        });
    }

    if let Some((source, values_per_class)) = find_discriminator(observations, &classes) {
        let rules = classes
            .iter()
            .zip(values_per_class)
            .map(|(class, value)| {
                let representative = class.last().expect("classe non vide");
                SuggestedRule {
                    method: method.to_string(),
                    sub_path: sub_path.to_string(),
                    condition: Some(Condition {
                        source: source.clone_with(value.clone()),
                        operator: Operator::Eq(value),
                    }),
                    sample_count: class.len(),
                    response: build_response(representative),
                }
            })
            .collect();
        return Some(Suggestion::Conditional { rules });
    }

    Some(Suggestion::VarianceUnexplained {
        sample_count: observations.len(),
        response_class_count: classes.len(),
    })
}

/// En-tetes de reponse jamais reportees dans une regle SUGGEREE (mais
/// conservees telles quelles dans `ObservedExchange` pour l'inspection brute)
/// : `content-length` est recalculee par le moteur de rendu a partir du
/// corps reel de la regle (une valeur figee deviendrait fausse des que
/// l'utilisateur edite le corps suggere), `date` fige un horodatage qui n'a
/// aucun sens une fois transforme en donnee statique de config.
const NEVER_SUGGESTED_RESPONSE_HEADERS: &[&str] = &["content-length", "date"];

fn build_response(exchange: &ObservedExchange) -> MockResponse {
    let headers = exchange
        .response_headers
        .iter()
        .filter(|(name, _)| {
            !NEVER_SUGGESTED_RESPONSE_HEADERS.contains(&name.to_lowercase().as_str())
        })
        .map(|(name, value)| HeaderEntry {
            name: name.clone(),
            value: value.clone(),
        })
        .collect();
    MockResponse {
        status: exchange.response_status,
        headers,
        body: vec![crate::models::BodyFragment::Literal {
            value: exchange.response_body.clone(),
        }],
        chaos: None,
    }
}

/// Gabarit d'une source de condition candidate, sans la valeur (connue
/// seulement une fois le champ retenu comme discriminant).
#[derive(Clone)]
enum SourceTemplate {
    QueryParam(String),
    JsonPointer(String),
    Header(String),
}

impl SourceTemplate {
    fn clone_with(&self, _value: String) -> ConditionSource {
        match self {
            SourceTemplate::QueryParam(k) => ConditionSource::QueryParam(k.clone()),
            SourceTemplate::JsonPointer(k) => ConditionSource::JsonPointer(format!("/{k}")),
            SourceTemplate::Header(k) => ConditionSource::Header(k.clone()),
        }
    }

    fn extract(
        &self,
        obs: &ObservedExchange,
        json_fields: &HashMap<String, String>,
    ) -> Option<String> {
        match self {
            SourceTemplate::QueryParam(k) => obs.request_query_params.get(k).cloned(),
            SourceTemplate::JsonPointer(k) => json_fields.get(k).cloned(),
            SourceTemplate::Header(k) => obs.request_headers.get(k).cloned(),
        }
    }
}

/// Parse le corps requete en JSON si c'est un objet a plat (pas de tableau/
/// objet imbrique en v1, cf commentaire de module) : chaque champ scalaire
/// devient un candidat `JsonPointer("/cle")`.
fn json_top_level_fields(body: &str) -> HashMap<String, String> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(body) else {
        return HashMap::new();
    };
    map.into_iter()
        .filter_map(|(k, v)| match v {
            serde_json::Value::String(s) => Some((k, s)),
            serde_json::Value::Number(n) => Some((k, n.to_string())),
            serde_json::Value::Bool(b) => Some((k, b.to_string())),
            _ => None,
        })
        .collect()
}

/// Cherche, PAR ORDRE DE PRIORITE (query param, puis champ JSON top-level de
/// requete, puis en-tete), un champ dont la valeur partitionne les
/// observations EXACTEMENT comme les classes de reponse deja calculees :
/// meme valeur => meme classe, classes differentes => valeurs differentes.
/// Retourne le gabarit de source retenu + la valeur representative de chaque
/// classe (meme ordre que `classes`).
fn find_discriminator(
    observations: &[ObservedExchange],
    classes: &[Vec<&ObservedExchange>],
) -> Option<(SourceTemplate, Vec<String>)> {
    let json_fields: Vec<HashMap<String, String>> = observations
        .iter()
        .map(|o| json_top_level_fields(&o.request_body))
        .collect();
    let json_fields_by_ptr = |obs_idx: usize| json_fields[obs_idx].clone();
    let _ = json_fields_by_ptr; // helper inutilise directement, cf boucle ci-dessous

    let mut query_keys = BTreeSet::new();
    let mut json_keys = BTreeSet::new();
    let mut header_keys = BTreeSet::new();
    for (i, obs) in observations.iter().enumerate() {
        query_keys.extend(obs.request_query_params.keys().cloned());
        json_keys.extend(json_fields[i].keys().cloned());
        header_keys.extend(
            obs.request_headers
                .keys()
                .filter(|k| !NOISY_HEADERS.contains(&k.to_lowercase().as_str()))
                .cloned(),
        );
    }

    let candidates: Vec<SourceTemplate> = query_keys
        .into_iter()
        .map(SourceTemplate::QueryParam)
        .chain(json_keys.into_iter().map(SourceTemplate::JsonPointer))
        .chain(header_keys.into_iter().map(SourceTemplate::Header))
        .collect();

    for candidate in candidates {
        if let Some(values) = discriminator_values(&candidate, classes, &json_fields, observations)
        {
            return Some((candidate, values));
        }
    }
    None
}

/// Valide UN candidat : chaque observation d'une meme classe doit porter la
/// MEME valeur pour ce champ (et la porter -- absence = candidat invalide),
/// et cette valeur doit etre UNIQUE a la classe (jamais partagee avec une
/// autre classe). Retourne la valeur representative de chaque classe si
/// valide.
fn discriminator_values(
    candidate: &SourceTemplate,
    classes: &[Vec<&ObservedExchange>],
    json_fields: &[HashMap<String, String>],
    all_observations: &[ObservedExchange],
) -> Option<Vec<String>> {
    let index_of = |obs: &ObservedExchange| -> usize {
        all_observations
            .iter()
            .position(|o| std::ptr::eq(o, obs))
            .expect("observation appartient au meme slice")
    };

    let mut values = Vec::with_capacity(classes.len());
    for class in classes {
        let mut class_value: Option<String> = None;
        for obs in class {
            let idx = index_of(obs);
            let v = candidate.extract(obs, &json_fields[idx])?;
            match &class_value {
                None => class_value = Some(v),
                Some(existing) if *existing != v => return None,
                Some(_) => {}
            }
        }
        values.push(class_value?);
    }

    let distinct: BTreeSet<&String> = values.iter().collect();
    if distinct.len() != values.len() {
        return None;
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn exchange(status: u16, body: &str) -> ObservedExchange {
        ObservedExchange::new(
            HashMap::new(),
            HashMap::new(),
            b"",
            Some("application/json".into()),
            status,
            HashMap::new(),
            body.as_bytes(),
            Some("application/json".into()),
        )
    }

    fn exchange_with_query(status: u16, body: &str, query: &[(&str, &str)]) -> ObservedExchange {
        let mut e = exchange(status, body);
        e.request_query_params = query
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        e
    }

    fn exchange_with_header(status: u16, body: &str, header: (&str, &str)) -> ObservedExchange {
        let mut e = exchange(status, body);
        e.request_headers
            .insert(header.0.to_string(), header.1.to_string());
        e
    }

    fn exchange_with_json_body_field(
        status: u16,
        resp_body: &str,
        req_field: (&str, &str),
    ) -> ObservedExchange {
        let mut e = exchange(status, resp_body);
        e.request_body = format!(r#"{{"{}":"{}"}}"#, req_field.0, req_field.1);
        e
    }

    #[test]
    fn below_min_samples_returns_none() {
        let obs = vec![exchange(200, "a"), exchange(200, "a")];
        assert!(suggest("GET", "/orders", &obs).is_none());
    }

    #[test]
    fn identical_responses_suggest_unconditional_rule() {
        let obs = vec![
            exchange(200, "ok"),
            exchange(200, "ok"),
            exchange(200, "ok"),
        ];
        let suggestion = suggest("GET", "/orders", &obs).unwrap();
        match suggestion {
            Suggestion::Unconditional { rule } => {
                assert!(rule.condition.is_none());
                assert_eq!(rule.sample_count, 3);
                assert_eq!(rule.response.status, 200);
            }
            other => panic!("expected Unconditional, got {other:?}"),
        }
    }

    #[test]
    fn suggested_response_never_includes_content_length_or_date() {
        // Vues sur le fil reel (proxy) : content-length et date sont
        // presentes cote reponse captured, mais n'ont pas leur place dans
        // une regle SAUVEGARDEE (content-length recalculee par le moteur de
        // rendu, date figerait un horodatage sans aucun sens en config
        // statique). Un en-tete "legitime" comme x-env doit lui survivre.
        let mut e = exchange(200, "ok");
        e.response_headers
            .insert("content-length".into(), "2".into());
        e.response_headers
            .insert("Date".into(), "Tue, 25 Aug 2026 00:00:00 GMT".into());
        e.response_headers.insert("x-env".into(), "prod".into());
        let obs = vec![e.clone(), e.clone(), e];

        let suggestion = suggest("GET", "/orders", &obs).unwrap();
        match suggestion {
            Suggestion::Unconditional { rule } => {
                let names: Vec<&str> = rule
                    .response
                    .headers
                    .iter()
                    .map(|h| h.name.as_str())
                    .collect();
                assert!(
                    !names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case("content-length"))
                );
                assert!(!names.iter().any(|n| n.eq_ignore_ascii_case("date")));
                assert!(names.contains(&"x-env"));
            }
            other => panic!("expected Unconditional, got {other:?}"),
        }
    }

    #[test]
    fn variance_explained_by_query_param_suggests_conditional_rules() {
        let obs = vec![
            exchange_with_query(200, r#"{"stock":true}"#, &[("id", "1")]),
            exchange_with_query(200, r#"{"stock":true}"#, &[("id", "1")]),
            exchange_with_query(404, "not found", &[("id", "2")]),
            exchange_with_query(404, "not found", &[("id", "2")]),
        ];
        let suggestion = suggest("GET", "/orders", &obs).unwrap();
        match suggestion {
            Suggestion::Conditional { rules } => {
                assert_eq!(rules.len(), 2);
                let statuses: Vec<u16> = rules.iter().map(|r| r.response.status).collect();
                assert!(statuses.contains(&200));
                assert!(statuses.contains(&404));
                for rule in &rules {
                    let cond = rule.condition.as_ref().unwrap();
                    assert_eq!(cond.source, ConditionSource::QueryParam("id".into()));
                }
            }
            other => panic!("expected Conditional, got {other:?}"),
        }
    }

    #[test]
    fn variance_explained_by_json_body_field() {
        let obs = vec![
            exchange_with_json_body_field(200, "actif", ("mode", "on")),
            exchange_with_json_body_field(200, "actif", ("mode", "on")),
            exchange_with_json_body_field(200, "inactif", ("mode", "off")),
            exchange_with_json_body_field(200, "inactif", ("mode", "off")),
        ];
        let suggestion = suggest("POST", "/toggle", &obs).unwrap();
        match suggestion {
            Suggestion::Conditional { rules } => {
                assert_eq!(rules.len(), 2);
                for rule in &rules {
                    let cond = rule.condition.as_ref().unwrap();
                    assert_eq!(cond.source, ConditionSource::JsonPointer("/mode".into()));
                }
            }
            other => panic!("expected Conditional, got {other:?}"),
        }
    }

    #[test]
    fn variance_explained_by_header() {
        let obs = vec![
            exchange_with_header(200, "prod-response", ("x-env", "prod")),
            exchange_with_header(200, "prod-response", ("x-env", "prod")),
            exchange_with_header(500, "staging-error", ("x-env", "staging")),
            exchange_with_header(500, "staging-error", ("x-env", "staging")),
        ];
        let suggestion = suggest("GET", "/status", &obs).unwrap();
        match suggestion {
            Suggestion::Conditional { rules } => {
                assert_eq!(rules.len(), 2);
                for rule in &rules {
                    let cond = rule.condition.as_ref().unwrap();
                    assert_eq!(cond.source, ConditionSource::Header("x-env".into()));
                }
            }
            other => panic!("expected Conditional, got {other:?}"),
        }
    }

    #[test]
    fn noisy_headers_are_never_used_as_discriminator() {
        // x-request-id varie a CHAQUE appel (unique par requete) mais n'a
        // aucune valeur predictive reelle -- ne doit jamais etre choisi,
        // meme s'il "discrimine parfaitement" au sens technique.
        let obs = vec![
            exchange_with_header(200, "ok", ("x-request-id", "r1")),
            exchange_with_header(200, "ok", ("x-request-id", "r2")),
            exchange_with_header(500, "err", ("x-request-id", "r3")),
        ];
        let suggestion = suggest("GET", "/status", &obs).unwrap();
        assert!(matches!(suggestion, Suggestion::VarianceUnexplained { .. }));
    }

    #[test]
    fn unexplainable_variance_suggests_nothing_actionable() {
        // Meme requete exacte (aucun champ ne varie), reponses differentes :
        // rien ne permet d'expliquer la variance -- ne JAMAIS figer une
        // regle inconditionnelle sur la premiere reponse observee.
        let obs = vec![
            exchange(200, "reponse-1"),
            exchange(200, "reponse-2"),
            exchange(200, "reponse-3"),
        ];
        let suggestion = suggest("GET", "/flaky", &obs).unwrap();
        match suggestion {
            Suggestion::VarianceUnexplained {
                sample_count,
                response_class_count,
            } => {
                assert_eq!(sample_count, 3);
                assert_eq!(response_class_count, 3);
            }
            other => panic!("expected VarianceUnexplained, got {other:?}"),
        }
    }

    #[test]
    fn partial_correlation_is_not_treated_as_discriminator() {
        // "id" vaut parfois la meme chose pour deux classes differentes :
        // ne discrimine PAS parfaitement, doit etre rejete comme candidat.
        let obs = vec![
            exchange_with_query(200, "a", &[("id", "1")]),
            exchange_with_query(404, "b", &[("id", "1")]),
            exchange_with_query(200, "a", &[("id", "2")]),
        ];
        let suggestion = suggest("GET", "/x", &obs).unwrap();
        assert!(matches!(suggestion, Suggestion::VarianceUnexplained { .. }));
    }
}
