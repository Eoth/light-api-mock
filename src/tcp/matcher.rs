// Matching sur octets bruts, first-match (meme philosophie que
// `engine::matcher` pour HTTP, mais sans reutilisation de code possible :
// ConditionSource/Operator sont modelises autour de valeurs textuelles
// extraites d'une requete HTTP structuree, ce qui n'existe pas ici -- on ne
// dispose que d'une tranche d'octets sans decodage protocolaire).
use crate::tcp::config::{TcpMatcher, TcpRule};
use crate::tcp::hex;

pub fn match_rule<'a>(rules: &'a [TcpRule], data: &[u8]) -> Option<&'a TcpRule> {
    rules.iter().find(|rule| matches(&rule.matcher, data))
}

fn matches(matcher: &TcpMatcher, data: &[u8]) -> bool {
    match matcher {
        TcpMatcher::Any => true,
        TcpMatcher::Prefix(expected_hex) => match hex::decode(expected_hex) {
            Ok(prefix) => !prefix.is_empty() && data.starts_with(&prefix),
            Err(_) => false,
        },
        // Compile a chaque appel, meme choix que Operator::Regex cote HTTP
        // (engine/matcher.rs) : pas de cache, coherence avec l'existant
        // plutot qu'une optimisation isolee sur ce seul chemin.
        TcpMatcher::Regex(pattern) => {
            regex::bytes::Regex::new(pattern).is_ok_and(|re| re.is_match(data))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, matcher: TcpMatcher) -> TcpRule {
        TcpRule {
            name: name.into(),
            matcher,
            response_hex: String::new(),
        }
    }

    #[test]
    fn any_matches_empty_data() {
        assert!(matches(&TcpMatcher::Any, &[]));
    }

    #[test]
    fn prefix_matches_when_data_starts_with_bytes() {
        let m = TcpMatcher::Prefix("300c".into());
        assert!(matches(&m, &[0x30, 0x0c, 0x02, 0x01]));
        assert!(!matches(&m, &[0x30, 0x0d]));
        assert!(!matches(&m, &[0x30]));
    }

    #[test]
    fn prefix_with_empty_hex_never_matches() {
        // Un prefixe vide matcherait n'importe quoi silencieusement si on
        // l'autorisait -- traite comme une regle mal configuree plutot que
        // comme un joker (celui-ci existe deja explicitement : TcpMatcher::Any).
        let m = TcpMatcher::Prefix(String::new());
        assert!(!matches(&m, &[0x30, 0x0c]));
        assert!(!matches(&m, &[]));
    }

    #[test]
    fn prefix_with_invalid_hex_never_matches() {
        let m = TcpMatcher::Prefix("zz".into());
        assert!(!matches(&m, &[0x30]));
    }

    #[test]
    fn regex_matches_arbitrary_bytes_not_just_utf8() {
        // (?-u) desactive le mode unicode de `regex::bytes` : necessaire pour
        // matcher des octets hors UTF-8 valide (trames binaires typiques).
        let m = TcpMatcher::Regex(r"(?-u)^\x30\x0c".into());
        assert!(matches(&m, &[0x30, 0x0c, 0xff]));
        assert!(!matches(&m, &[0x31, 0x0c]));
    }

    #[test]
    fn regex_invalid_pattern_never_matches_never_panics() {
        let m = TcpMatcher::Regex("(unclosed".into());
        assert!(!matches(&m, &[0x30]));
    }

    #[test]
    fn match_rule_returns_first_match_in_order() {
        let rules = vec![
            rule("specific", TcpMatcher::Prefix("30".into())),
            rule("fallback", TcpMatcher::Any),
        ];
        let matched = match_rule(&rules, &[0x30, 0xff]);
        assert_eq!(matched.unwrap().name, "specific");

        let matched = match_rule(&rules, &[0x99]);
        assert_eq!(matched.unwrap().name, "fallback");
    }

    #[test]
    fn match_rule_returns_none_when_nothing_matches() {
        let rules = vec![rule("only", TcpMatcher::Prefix("30".into()))];
        assert!(match_rule(&rules, &[0x99]).is_none());
    }

    #[test]
    fn match_rule_empty_rules_returns_none() {
        assert!(match_rule(&[], &[0x30]).is_none());
    }
}
