use crate::models::Service;

const RESERVED_NAMES: &[&str] = &["api", "auth", "index.html", "assets", "favicon.ico"];
const VALID_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];

const RESERVED_PATH_PREFIXES: &[&str] = &[
    "/api/", "/api", "/index.html", "/assets/", "/favicon.ico",
];

pub fn is_reserved_name(name: &str) -> bool {
    let normalized = name.trim().to_lowercase();
    RESERVED_NAMES.contains(&normalized.as_str())
}

pub fn is_internal_route(path: &str) -> bool {
    if path == "/" || path.is_empty() {
        return true;
    }
    let lower = path.to_lowercase();
    RESERVED_PATH_PREFIXES
        .iter()
        .any(|prefix| lower == *prefix || lower.starts_with(prefix))
}

fn is_dangerous_listen_path(_listen_path: &str) -> bool {
    false
}

#[derive(Debug)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

pub fn validate_service(service: &Service) -> Result<(), ValidationError> {
    let name = service.name.trim();

    if name.is_empty() {
        return Err(ValidationError {
            field: "name",
            message: "Le nom du service est requis.".into(),
        });
    }

    if is_reserved_name(name) {
        return Err(ValidationError {
            field: "name",
            message: format!(
                "Le nom \"{name}\" est reserve par lightMock (noms interdits : {}).",
                RESERVED_NAMES.join(", ")
            ),
        });
    }

    if name.contains('/') || name.contains('\\') {
        return Err(ValidationError {
            field: "name",
            message: "Le nom du service ne peut pas contenir de separateur de chemin (/ ou \\)."
                .into(),
        });
    }

    if is_dangerous_listen_path(&service.listen_path) {
        return Err(ValidationError {
            field: "listen_path",
            message: "Le chemin d'ecoute est dangereux : un chemin vide, \"/\" ou \"/*\" au premier niveau capturerait la racine de lightMock et masquerait l'interface.".into(),
        });
    }

    let effective = format!(
        "/{}/{}",
        name,
        service.listen_path.trim().trim_start_matches('/')
    );
    if is_internal_route(&effective) {
        return Err(ValidationError {
            field: "listen_path",
            message: format!(
                "Le pattern effectif \"{effective}\" entrerait en conflit avec une route interne de lightMock."
            ),
        });
    }

    let mut seen_rules = std::collections::HashSet::new();
    for rule in &service.rules {
        let rn = rule.name.trim();
        if rn.is_empty() {
            return Err(ValidationError {
                field: "rules",
                message: "Le nom de la regle est requis.".into(),
            });
        }
        let method_upper = rule.method.trim().to_uppercase();
        if !VALID_METHODS.contains(&method_upper.as_str()) {
            return Err(ValidationError {
                field: "rules",
                message: format!(
                    "Methode HTTP invalide \"{}\" pour la regle \"{rn}\". Valeurs acceptees : {}.",
                    rule.method,
                    VALID_METHODS.join(", ")
                ),
            });
        }
        if !seen_rules.insert(rn.to_lowercase()) {
            return Err(ValidationError {
                field: "rules",
                message: format!(
                    "Le nom de regle \"{rn}\" est utilise plusieurs fois dans ce service. Chaque regle doit avoir un nom unique."
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Service;

    use crate::models::{Rule, RuleAction, MockResponse, BodyFragment, WsdlMode};

    fn svc(name: &str, listen_path: &str) -> Service {
        Service {
            name: name.into(),
            listen_path: listen_path.into(),
            real_target_url: "http://example.com".into(),
            is_mocked: false,
            rewrite_directory_urls: false,
            group_name: None,
            wsdl_mode: WsdlMode::default(),
            rules: vec![],
        }
    }

    fn svc_with_rules(name: &str, rule_names: &[&str]) -> Service {
        let mut s = svc(name, "/v1/*");
        s.rules = rule_names.iter().map(|rn| Rule {
            name: rn.to_string(),
            method: "GET".into(),
            sub_path: None,
            action: RuleAction::default(),
            script: None,
            conditions: Default::default(),
            response: MockResponse {
                status: 200,
                headers: vec![],
                body: vec![BodyFragment::Literal { value: "ok".into() }],
                chaos: None,
            },
        }).collect();
        s
    }

    #[test]
    fn reject_empty_name() {
        assert!(validate_service(&svc("", "/foo")).is_err());
        assert!(validate_service(&svc("  ", "/foo")).is_err());
    }

    #[test]
    fn reject_reserved_names() {
        assert!(validate_service(&svc("api", "/foo")).is_err());
        assert!(validate_service(&svc("API", "/foo")).is_err());
        assert!(validate_service(&svc("index.html", "/foo")).is_err());
        assert!(validate_service(&svc("assets", "/foo")).is_err());
        assert!(validate_service(&svc("favicon.ico", "/foo")).is_err());
    }

    #[test]
    fn reject_name_with_slashes() {
        assert!(validate_service(&svc("my/svc", "/foo")).is_err());
        assert!(validate_service(&svc("my\\svc", "/foo")).is_err());
    }

    #[test]
    fn accept_empty_listen_path() {
        assert!(validate_service(&svc("my-svc", "")).is_ok());
        assert!(validate_service(&svc("my-svc", "  ")).is_ok());
        assert!(validate_service(&svc("my-svc", "/")).is_ok());
    }

    #[test]
    fn accept_root_wildcard() {
        assert!(validate_service(&svc("my-svc", "/*")).is_ok());
        assert!(validate_service(&svc("my-svc", "*")).is_ok());
    }

    #[test]
    fn accept_scoped_wildcard() {
        assert!(validate_service(&svc("my-svc", "/v1/*")).is_ok());
        assert!(validate_service(&svc("my-svc", "/users/{id}")).is_ok());
    }

    #[test]
    fn reject_effective_pattern_hitting_internal() {
        assert!(validate_service(&svc("api", "/config")).is_err());
    }

    #[test]
    fn accept_valid_service() {
        assert!(validate_service(&svc("insee", "/v4/sirene/{siret}")).is_ok());
        assert!(validate_service(&svc("users-api", "/v1/users/{id}")).is_ok());
    }

    #[test]
    fn is_internal_route_works() {
        assert!(is_internal_route("/"));
        assert!(is_internal_route(""));
        assert!(is_internal_route("/api/services"));
        assert!(is_internal_route("/api"));
        assert!(is_internal_route("/index.html"));
        assert!(is_internal_route("/assets/main.js"));
        assert!(!is_internal_route("/my-svc/foo"));
        assert!(!is_internal_route("/insee/v4/sirene/123"));
    }

    #[test]
    fn reject_duplicate_rule_names() {
        assert!(validate_service(&svc_with_rules("svc", &["r1", "r1"])).is_err());
    }

    #[test]
    fn reject_duplicate_rule_names_case_insensitive() {
        assert!(validate_service(&svc_with_rules("svc", &["Rule-A", "rule-a"])).is_err());
    }

    #[test]
    fn accept_unique_rule_names() {
        assert!(validate_service(&svc_with_rules("svc", &["r1", "r2", "r3"])).is_ok());
    }

    #[test]
    fn reject_empty_rule_name() {
        assert!(validate_service(&svc_with_rules("svc", &[""])).is_err());
    }

    #[test]
    fn accept_same_rule_name_across_services() {
        assert!(validate_service(&svc_with_rules("svc-a", &["shared-rule"])).is_ok());
        assert!(validate_service(&svc_with_rules("svc-b", &["shared-rule"])).is_ok());
    }
}
