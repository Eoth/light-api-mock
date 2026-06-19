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
}

impl MatchEngine {
    pub fn first_match<'a>(rules: &'a [Rule], req: &RequestData) -> Option<&'a Rule> {
        rules.iter().find(|rule| Self::matches_group(&rule.conditions, req))
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
            query_params: query.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            body: body.to_vec(),
            content_type: ct.map(String::from),
            path_params: HashMap::new(),
        }
    }

    fn simple_rule(name: &str, conditions: ConditionGroup) -> Rule {
        Rule {
            name: name.into(),
            conditions,
            response: MockResponse {
                status: 200,
                headers: vec![],
                body: vec![BodyFragment::Literal { value: name.into() }],
                chaos: None,
            },
        }
    }

    #[test]
    fn first_match_returns_first_matching_rule() {
        let rules = vec![
            simple_rule("r1", ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("x-env".into()),
                    operator: Operator::Eq("prod".into()),
                }],
                any_of: vec![],
            }),
            simple_rule("r2", ConditionGroup {
                all_of: vec![Condition {
                    source: ConditionSource::Header("x-env".into()),
                    operator: Operator::Eq("staging".into()),
                }],
                any_of: vec![],
            }),
        ];
        let req = make_req(&[], &[("x-env", "staging")], b"", None);
        let matched = MatchEngine::first_match(&rules, &req);
        assert_eq!(matched.unwrap().name, "r2");
    }

    #[test]
    fn no_match_returns_none() {
        let rules = vec![simple_rule("r1", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::Header("x-env".into()),
                operator: Operator::Eq("prod".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[], &[("x-env", "dev")], b"", None);
        assert!(MatchEngine::first_match(&rules, &req).is_none());
    }

    #[test]
    fn empty_conditions_always_match() {
        let rules = vec![simple_rule("catch-all", ConditionGroup::default())];
        let req = make_req(&[], &[], b"", None);
        assert_eq!(MatchEngine::first_match(&rules, &req).unwrap().name, "catch-all");
    }

    #[test]
    fn query_param_eq() {
        let rules = vec![simple_rule("qp", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::QueryParam("id".into()),
                operator: Operator::Eq("42".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[("id", "42")], &[], b"", None);
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn header_case_insensitive() {
        let rules = vec![simple_rule("hdr", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::Header("Content-Type".into()),
                operator: Operator::Contains("json".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[], &[("content-type", "application/json")], b"", None);
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn json_pointer_extraction() {
        let body = br#"{"user":{"role":"admin","id":5}}"#;
        let rules = vec![simple_rule("jp", ConditionGroup {
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
        })];
        let req = make_req(&[], &[], body, Some("application/json"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn xpath_extraction() {
        let body = br#"<Envelope><Body><id>123</id></Body></Envelope>"#;
        let rules = vec![simple_rule("xp", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::XPath("Envelope/Body/id".into()),
                operator: Operator::Eq("123".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[], &[], body, Some("text/xml"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn xpath_with_namespace() {
        let body = br#"<soap:Envelope><soap:Body><ns:id>abc</ns:id></soap:Body></soap:Envelope>"#;
        let rules = vec![simple_rule("xpns", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::XPath("Envelope/Body/id".into()),
                operator: Operator::Eq("abc".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[], &[], body, Some("text/xml"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn form_field_extraction() {
        let body = b"username=admin&password=secret";
        let rules = vec![simple_rule("form", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::FormField("username".into()),
                operator: Operator::Eq("admin".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[], &[], body, Some("application/x-www-form-urlencoded"));
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn body_raw_contains() {
        let body = b"Hello World test payload";
        let rules = vec![simple_rule("raw", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::BodyRaw,
                operator: Operator::Contains("World".into()),
            }],
            any_of: vec![],
        })];
        let req = make_req(&[], &[], body, None);
        assert!(MatchEngine::first_match(&rules, &req).is_some());
    }

    #[test]
    fn regex_operator() {
        let rules = vec![simple_rule("rgx", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::QueryParam("code".into()),
                operator: Operator::Regex(r"^\d{3}$".into()),
            }],
            any_of: vec![],
        })];
        let yes = make_req(&[("code", "200")], &[], b"", None);
        let no = make_req(&[("code", "abcd")], &[], b"", None);
        assert!(MatchEngine::first_match(&rules, &yes).is_some());
        assert!(MatchEngine::first_match(&rules, &no).is_none());
    }

    #[test]
    fn exists_operator() {
        let rules = vec![simple_rule("ex", ConditionGroup {
            all_of: vec![Condition {
                source: ConditionSource::Header("x-debug".into()),
                operator: Operator::Exists,
            }],
            any_of: vec![],
        })];
        let yes = make_req(&[], &[("x-debug", "")], b"", None);
        let no = make_req(&[], &[], b"", None);
        assert!(MatchEngine::first_match(&rules, &yes).is_some());
        assert!(MatchEngine::first_match(&rules, &no).is_none());
    }

    #[test]
    fn all_of_and_any_of_combined() {
        let rules = vec![simple_rule("combo", ConditionGroup {
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
        })];

        let ok1 = make_req(&[("debug", "1")], &[("x-env", "staging")], b"", None);
        let ok2 = make_req(&[("trace", "1")], &[("x-env", "staging")], b"", None);
        let fail_header = make_req(&[("debug", "1")], &[("x-env", "prod")], b"", None);
        let fail_any = make_req(&[], &[("x-env", "staging")], b"", None);

        assert!(MatchEngine::first_match(&rules, &ok1).is_some());
        assert!(MatchEngine::first_match(&rules, &ok2).is_some());
        assert!(MatchEngine::first_match(&rules, &fail_header).is_none());
        assert!(MatchEngine::first_match(&rules, &fail_any).is_none());
    }
}
