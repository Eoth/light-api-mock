// Schema + chargement du mock/proxy TCP brut. Delibrement HORS de
// `models::MockConfig` (contrairement a une premiere version de cette
// tranche) : ce module aurait alors force `MockConfig` a exposer un
// nouveau champ, obligatoire dans TOUT literal `MockConfig { .. }` du
// crate (~20 sites, presque tous dans des tests HTTP existants deja en
// production) — un rayon d'impact que "brique separee" est cense eviter
// par construction. Meme choix que `messaging::KafkaConfig`, qui ne touche
// jamais non plus a `MockConfig`.
//
// Consequence assumee pour cette premiere tranche : la config TCP vit dans
// son PROPRE fichier YAML (`tcp-config.yaml`, meme repertoire `DATA_PATH`
// que `mock-config.yaml`), chargee UNE FOIS au demarrage, pas de
// write-behind/backup/rotation (ce que fournit `MockStore` pour la config
// HTTP) tant qu'aucune API ne permet de la modifier a chaud -- rien a
// proteger en ecriture qui n'existe pas encore.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TcpConfig {
    #[serde(default)]
    pub services: Vec<TcpService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TcpService {
    pub name: String,
    pub listen_port: u16,
    /// "host:port" de la vraie cible. Requis des qu'une regle utilise
    /// `TcpRuleAction::Proxy`, ou si `is_mocked` est false (proxy direct,
    /// meme convention que `Service::is_mocked` cote HTTP).
    #[serde(default)]
    pub real_target_addr: Option<String>,
    #[serde(default = "default_true")]
    pub is_mocked: bool,
    #[serde(default)]
    pub rules: Vec<TcpRule>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TcpRule {
    pub name: String,
    pub matcher: TcpMatcher,
    #[serde(default)]
    pub action: TcpRuleAction,
    /// Reponse brute renvoyee au client si `action == Mock`. Ignore si
    /// `action == Proxy`. Encodage hexadecimal (pas base64) : evite une
    /// dependance externe supplementaire, cf `tcp::hex`.
    #[serde(default)]
    pub response_hex: String,
}

/// Strategie de matching sur les octets bruts recus a l'ouverture de la
/// connexion. Premiere regle qui matche gagne (meme philosophie first-match
/// que `engine::matcher` cote HTTP), evaluee dans l'ordre de
/// `TcpService::rules` — cf `tcp::matcher`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum TcpMatcher {
    /// Prefixe attendu, encode en hexadecimal (ex: trame BER LDAP
    /// commencant par 0x30 = SEQUENCE).
    Prefix(String),
    /// Pattern `regex::bytes::Regex` applique aux octets bruts (pas d'UTF-8
    /// garanti).
    Regex(String),
    /// Matche toujours : utile comme regle de repli en fin de liste.
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TcpRuleAction {
    #[default]
    Mock,
    Proxy,
}

impl TcpConfig {
    pub fn config_file(data_dir: &Path) -> PathBuf {
        data_dir.join("tcp-config.yaml")
    }

    /// Absence de fichier = config vide (feature activee mais rien a
    /// ecouter), PAS une erreur : contrairement a `MockStore::load_or_init`,
    /// ce fichier n'est jamais auto-cree -- il n'existe encore aucune UI/API
    /// pour le faire naitre, l'utilisateur qui veut s'en servir aujourd'hui
    /// l'ecrit lui-meme a la main dans `DATA_PATH`.
    ///
    /// Un fichier present mais illisible/mal forme est logge en erreur et
    /// degrade vers une config vide plutot que de faire echouer le
    /// demarrage : contrairement au YAML HTTP (le produit principal, deja
    /// en production), cette fonctionnalite est encore une premiere
    /// tranche -- une erreur dedans ne doit jamais empecher le serveur HTTP
    /// de demarrer.
    pub fn load(data_dir: &Path) -> Self {
        let file = Self::config_file(data_dir);
        if !file.exists() {
            return Self::default();
        }
        let content = match std::fs::read_to_string(&file) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(path = %file.display(), error = %e, "tcp-proxy: failed to read tcp-config.yaml, falling back to empty config");
                return Self::default();
            }
        };
        match serde_yaml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::error!(path = %file.display(), error = %e, "tcp-proxy: failed to parse tcp-config.yaml, falling back to empty config");
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_empty_config() {
        let dir =
            std::env::temp_dir().join(format!("lightmock-tcp-cfg-test-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        let config = TcpConfig::load(&dir);
        assert!(config.services.is_empty());
    }

    #[test]
    fn load_malformed_yaml_falls_back_to_empty_config_without_panicking() {
        let dir =
            std::env::temp_dir().join(format!("lightmock-tcp-cfg-test-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            TcpConfig::config_file(&dir),
            "not: [valid, yaml: for-this-schema",
        )
        .unwrap();
        let config = TcpConfig::load(&dir);
        assert!(config.services.is_empty());
    }

    #[test]
    fn roundtrip_yaml_serialization() {
        let config = TcpConfig {
            services: vec![TcpService {
                name: "ldap-directory".into(),
                listen_port: 3890,
                real_target_addr: Some("ldap.internal:389".into()),
                is_mocked: true,
                rules: vec![
                    TcpRule {
                        name: "bind-request".into(),
                        matcher: TcpMatcher::Prefix("300c02010060070201".into()),
                        action: TcpRuleAction::Mock,
                        response_hex: "300c02010161070a010000040400".into(),
                    },
                    TcpRule {
                        name: "fallback".into(),
                        matcher: TcpMatcher::Any,
                        action: TcpRuleAction::Proxy,
                        response_hex: String::new(),
                    },
                ],
            }],
        };
        let yaml = serde_yaml::to_string(&config).expect("serialize");
        let parsed: TcpConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn load_reads_real_file_from_disk() {
        let dir =
            std::env::temp_dir().join(format!("lightmock-tcp-cfg-test-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        let yaml = r#"
services:
  - name: svc
    listen_port: 9999
    is_mocked: true
    rules:
      - name: r
        matcher: { type: Any }
"#;
        std::fs::write(TcpConfig::config_file(&dir), yaml).unwrap();
        let config = TcpConfig::load(&dir);
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.services[0].listen_port, 9999);
        assert_eq!(config.services[0].rules[0].action, TcpRuleAction::Mock);
    }

    #[test]
    fn tcp_rule_action_defaults_to_mock_and_response_hex_defaults_to_empty() {
        let yaml = r#"
name: r
matcher: { type: Any }
"#;
        let rule: TcpRule = serde_yaml::from_str(yaml).expect("deserialize");
        assert_eq!(rule.action, TcpRuleAction::Mock);
        assert_eq!(rule.response_hex, "");
    }

    #[test]
    fn is_mocked_defaults_to_true() {
        let yaml = r#"
name: svc
listen_port: 1
rules: []
"#;
        let service: TcpService = serde_yaml::from_str(yaml).expect("deserialize");
        assert!(service.is_mocked);
    }
}
