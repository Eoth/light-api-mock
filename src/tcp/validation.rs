// Validation d'un TcpService avant ecriture dans TcpRuntime, meme forme que
// `server::validation::validate_service` cote HTTP (ValidationError { field,
// message }) mais schema totalement different (pas de listen_path/method,
// port + regles a octets bruts a la place).
use crate::i18n::tr;
use crate::tcp::config::{TcpMatcher, TcpService};
use crate::tcp::hex;

static NAME_CHARSET_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Za-z0-9_-]+$").unwrap());

#[derive(Debug)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

/// `existing` : les autres services deja configures, DEJA EXCLUS de celui en
/// cours de validation cote appelant (cf `server/api.rs` -- exclut le
/// service qu'on est en train de mettre a jour de la verification d'unicite,
/// sinon un PUT qui ne change rien se rejetterait lui-meme).
pub fn validate_tcp_service(
    service: &TcpService,
    existing: &[TcpService],
) -> Result<(), ValidationError> {
    let name = service.name.trim();
    let error = |field, message| Err(ValidationError { field, message });

    if name.is_empty() {
        return error("name", tr("The service name is required.", &[]));
    }
    if !NAME_CHARSET_RE.is_match(name) {
        return error(
            "name",
            tr(
                "A service name can only contain letters, digits, dashes (-) and underscores (_).",
                &[],
            ),
        );
    }
    if existing.iter().any(|s| s.name == name) {
        return error(
            "name",
            tr("A TCP service named \"{0}\" already exists.", &[&name]),
        );
    }
    if existing
        .iter()
        .any(|s| s.listen_port == service.listen_port)
    {
        return error(
            "listen_port",
            tr(
                "The port {0} is already used by another TCP service.",
                &[&service.listen_port],
            ),
        );
    }

    for (i, rule) in service.rules.iter().enumerate() {
        if rule.name.trim().is_empty() {
            return error("rules", tr("The rule #{0} has no name.", &[&i]));
        }
        match &rule.matcher {
            TcpMatcher::Prefix(hex_str) if hex::decode(hex_str).is_err() => {
                return error(
                    "rules",
                    tr(
                        "Rule \"{0}\": the prefix is not valid hexadecimal.",
                        &[&rule.name],
                    ),
                );
            }
            TcpMatcher::Regex(pattern)
                if crate::engine::regex_cache::check_bytes(pattern).is_err() =>
            {
                return error(
                    "rules",
                    tr("Rule \"{0}\": invalid regex pattern.", &[&rule.name]),
                );
            }
            _ => {}
        }
        if hex::decode(&rule.response_hex).is_err() {
            return error(
                "rules",
                tr(
                    "Rule \"{0}\": the response is not valid hexadecimal.",
                    &[&rule.name],
                ),
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::config::TcpRule;

    fn valid_service() -> TcpService {
        TcpService {
            name: "svc".into(),
            listen_port: 9000,
            rules: vec![TcpRule {
                name: "r".into(),
                matcher: TcpMatcher::Any,
                response_hex: "706f6e67".into(),
            }],
        }
    }

    #[test]
    fn accepts_valid_service() {
        assert!(validate_tcp_service(&valid_service(), &[]).is_ok());
    }

    #[test]
    fn rejects_empty_name() {
        let mut s = valid_service();
        s.name = "  ".into();
        let e = validate_tcp_service(&s, &[]).unwrap_err();
        assert_eq!(e.field, "name");
    }

    #[test]
    fn rejects_name_with_invalid_chars() {
        let mut s = valid_service();
        s.name = "not a name!".into();
        let e = validate_tcp_service(&s, &[]).unwrap_err();
        assert_eq!(e.field, "name");
    }

    #[test]
    fn rejects_duplicate_name() {
        let existing = vec![valid_service()];
        let mut s = valid_service();
        s.listen_port = 9001;
        let e = validate_tcp_service(&s, &existing).unwrap_err();
        assert_eq!(e.field, "name");
    }

    #[test]
    fn rejects_duplicate_port() {
        let existing = vec![valid_service()];
        let mut s = valid_service();
        s.name = "other".into();
        let e = validate_tcp_service(&s, &existing).unwrap_err();
        assert_eq!(e.field, "listen_port");
    }

    #[test]
    fn rejects_rule_without_name() {
        let mut s = valid_service();
        s.rules[0].name = "".into();
        let e = validate_tcp_service(&s, &[]).unwrap_err();
        assert_eq!(e.field, "rules");
    }

    #[test]
    fn rejects_invalid_prefix_hex() {
        let mut s = valid_service();
        s.rules[0].matcher = TcpMatcher::Prefix("zz".into());
        let e = validate_tcp_service(&s, &[]).unwrap_err();
        assert_eq!(e.field, "rules");
    }

    #[test]
    fn rejects_invalid_regex_pattern() {
        let mut s = valid_service();
        s.rules[0].matcher = TcpMatcher::Regex("(unclosed".into());
        let e = validate_tcp_service(&s, &[]).unwrap_err();
        assert_eq!(e.field, "rules");
    }

    #[test]
    fn rejects_invalid_response_hex() {
        let mut s = valid_service();
        s.rules[0].response_hex = "zz".into();
        let e = validate_tcp_service(&s, &[]).unwrap_err();
        assert_eq!(e.field, "rules");
    }

    #[test]
    fn empty_response_hex_is_valid() {
        // Une regle qui matche et repond "rien" (fermeture immediate) est un
        // cas legitime, pas une erreur.
        let mut s = valid_service();
        s.rules[0].response_hex = String::new();
        assert!(validate_tcp_service(&s, &[]).is_ok());
    }
}
