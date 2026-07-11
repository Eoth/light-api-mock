// Adaptation de MatchEngine (src/engine/matcher.rs) pour matcher un message
// Kafka (payload + headers) au lieu d'une requete HTTP.
//
// Reutilisation : `MatchEngine::matches_group()` (rendue pub(crate)
// specifiquement pour cet usage, voir son commentaire) evalue les conditions
// all_of/any_of d'une regle avec les MEMES extracteurs que le HTTP
// (QueryParam, Header, JsonPointer, XPath, FormField, PathParam, BodyRaw) —
// aucune logique d'extraction/evaluation n'est dupliquee ici.
//
// Adaptation : on saute deliberement `matches_method`/`matches_sub_path`. Un
// message Kafka n'a ni verbe HTTP ni chemin — `Rule.method`/`Rule.sub_path`
// restent des champs obligatoires du modele (partage avec les regles HTTP,
// pas de champ optionnel a la legere), mais ils sont
// simplement ignores lors du matching de message : seules `Rule.conditions`
// comptent. `query_params`/`path_params` sont toujours vides dans le
// `RequestData` synthetique construit ici (pas d'equivalent Kafka), donc une
// condition qui en depend ne matchera jamais un message — attendu.
//
// Perimetre de service : KafkaConfig ne modelise qu'UN topic d'ecoute global
// (pas de scoping par service comme listen_path pour HTTP), donc "le service
// concerne" est determine en parcourant TOUS les services dans l'ordre de
// la config, premiere regle matchee (first-match, meme philosophie que HTTP)
// — pas de nouveau champ obligatoire ajoute a Service pour cette passe.
use crate::engine::{MatchEngine, RequestData};
use crate::models::{Rule, Service};
use std::collections::HashMap;

pub struct MatchedMessage<'a> {
    pub service_name: &'a str,
    pub rule: &'a Rule,
}

pub fn match_message<'a>(
    services: &'a [Service],
    payload: &[u8],
    headers: &HashMap<String, String>,
) -> Option<MatchedMessage<'a>> {
    let req = RequestData {
        query_params: HashMap::new(),
        headers: headers.clone(),
        body: payload.to_vec(),
        content_type: headers.get("content-type").cloned(),
        path_params: HashMap::new(),
        method: String::new(),
        remaining_path: String::new(),
    };

    for service in services {
        if let Some(rule) = service
            .rules
            .iter()
            .find(|r| MatchEngine::matches_group(&r.conditions, &req))
        {
            return Some(MatchedMessage {
                service_name: &service.name,
                rule,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn service_with_rule(name: &str, conditions: ConditionGroup, body_literal: &str) -> Service {
        Service {
            name: name.into(),
            listen_path: String::new(),
            real_target_url: "http://unused".into(),
            is_mocked: true,
            rewrite_directory_urls: false,
            group_name: None,
            wsdl_mode: WsdlMode::default(),
            rules: vec![Rule {
                name: format!("{name}-rule"),
                method: "ANY".into(),
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
                        value: body_literal.into(),
                    }],
                    chaos: None,
                },
            }],
        }
    }

    #[test]
    fn matches_on_json_pointer_condition() {
        let services = vec![service_with_rule(
            "svc-a",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::JsonPointer("/type".into()),
                    operator: Operator::Eq("order.created".into()),
                }],
                any_of: vec![],
            },
            "ok",
        )];
        let payload = br#"{"type":"order.created"}"#;
        let m = match_message(&services, payload, &HashMap::new()).unwrap();
        assert_eq!(m.service_name, "svc-a");
        assert_eq!(m.rule.name, "svc-a-rule");
    }

    #[test]
    fn matches_on_header_condition() {
        let services = vec![service_with_rule(
            "svc-a",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("event-type".into()),
                    operator: Operator::Eq("created".into()),
                }],
                any_of: vec![],
            },
            "ok",
        )];
        let mut headers = HashMap::new();
        headers.insert("event-type".to_string(), "created".to_string());
        let m = match_message(&services, b"{}", &headers).unwrap();
        assert_eq!(m.service_name, "svc-a");
    }

    #[test]
    fn no_match_returns_none() {
        let services = vec![service_with_rule(
            "svc-a",
            ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::JsonPointer("/type".into()),
                    operator: Operator::Eq("order.created".into()),
                }],
                any_of: vec![],
            },
            "ok",
        )];
        let payload = br#"{"type":"order.cancelled"}"#;
        assert!(match_message(&services, payload, &HashMap::new()).is_none());
    }

    #[test]
    fn empty_conditions_always_match() {
        let services = vec![service_with_rule("catch-all", ConditionGroup::default(), "ok")];
        let m = match_message(&services, b"anything", &HashMap::new()).unwrap();
        assert_eq!(m.service_name, "catch-all");
    }

    #[test]
    fn first_matching_service_wins_across_multiple_services() {
        let services = vec![
            service_with_rule(
                "svc-a",
                ConditionGroup {
                    all_of: vec![Condition {
                        source: ConditionSource::JsonPointer("/type".into()),
                        operator: Operator::Eq("nope".into()),
                    }],
                    any_of: vec![],
                },
                "a",
            ),
            service_with_rule("svc-b", ConditionGroup::default(), "b"),
        ];
        let m = match_message(&services, b"{}", &HashMap::new()).unwrap();
        assert_eq!(m.service_name, "svc-b");
    }

    #[test]
    fn method_and_sub_path_are_ignored_for_messages() {
        let mut svc = service_with_rule("svc-a", ConditionGroup::default(), "ok");
        svc.rules[0].method = "DELETE".into();
        svc.rules[0].sub_path = Some("/never/matches/a/message".into());
        let services = [svc];
        let m = match_message(&services, b"{}", &HashMap::new());
        assert!(m.is_some(), "method/sub_path must not gate message matching");
    }

    #[test]
    fn no_services_returns_none() {
        assert!(match_message(&[], b"{}", &HashMap::new()).is_none());
    }
}
