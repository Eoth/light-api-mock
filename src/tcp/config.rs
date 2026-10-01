// Schema and loading of the raw TCP mocks. Kept out of `models::MockConfig` on purpose: a new field there would be
// required in every `MockConfig { .. }` literal of the crate (about twenty, nearly all in HTTP tests) for a feature
// most builds do not have, like `messaging::KafkaConfig`.
//
// So the TCP configuration has its own file, `tcp-config.yaml`, next to `mock-config.yaml` in `DATA_PATH`, written
// atomically by `TcpRuntime::replace()`, without the backups and write-behind of `MockStore`: changes are rare and
// small.
//
// No proxy mode (see `tcp::mod`): a `TcpService` has mock rules only, never a target.
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
    #[serde(default)]
    pub rules: Vec<TcpRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TcpRule {
    pub name: String,
    pub matcher: TcpMatcher,
    /// Raw bytes sent back when this rule matches, as hexadecimal text (see `tcp::hex`).
    #[serde(default)]
    pub response_hex: String,
}

/// How a rule matches the raw bytes received when the connection opens. Rules are tried in the order of
/// `TcpService::rules` and the first match wins, as for HTTP (see `tcp::matcher`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum TcpMatcher {
    /// Expected prefix, as hexadecimal text.
    Prefix(String),
    /// A `regex::bytes::Regex` applied to the raw bytes, which need not be UTF-8.
    Regex(String),
    /// Always matches: a fallback rule at the end of the list.
    Any,
}

impl TcpConfig {
    pub fn config_file(data_dir: &Path) -> PathBuf {
        data_dir.join("tcp-config.yaml")
    }

    /// No file means no TCP service, not an error: the file is created by the first change through the API.
    ///
    /// A file that cannot be read or parsed is logged as an error and treated as empty rather than stopping the
    /// start: an optional feature must never keep the HTTP server from starting.
    pub fn load(data_dir: &Path) -> Self {
        let file = Self::config_file(data_dir);
        if !file.exists() {
            return Self::default();
        }
        let content = match std::fs::read_to_string(&file) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(path = %file.display(), error = %e, "tcp-mock: failed to read tcp-config.yaml, falling back to empty config");
                return Self::default();
            }
        };
        match serde_yaml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::error!(path = %file.display(), error = %e, "tcp-mock: failed to parse tcp-config.yaml, falling back to empty config");
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
        let dir = crate::server::test_support::temp_data_dir("tcp-cfg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let config = TcpConfig::load(&dir);
        assert!(config.services.is_empty());
    }

    #[test]
    fn load_malformed_yaml_falls_back_to_empty_config_without_panicking() {
        let dir = crate::server::test_support::temp_data_dir("tcp-cfg-test");
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
                name: "heartbeat-mock".into(),
                listen_port: 9000,
                rules: vec![
                    TcpRule {
                        name: "ping".into(),
                        matcher: TcpMatcher::Prefix("50494e47".into()),
                        response_hex: "504f4e47".into(),
                    },
                    TcpRule {
                        name: "fallback".into(),
                        matcher: TcpMatcher::Any,
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
        let dir = crate::server::test_support::temp_data_dir("tcp-cfg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let yaml = r#"
services:
  - name: svc
    listen_port: 9999
    rules:
      - name: r
        matcher: { type: Any }
"#;
        std::fs::write(TcpConfig::config_file(&dir), yaml).unwrap();
        let config = TcpConfig::load(&dir);
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.services[0].listen_port, 9999);
    }

    #[test]
    fn response_hex_defaults_to_empty() {
        let yaml = r#"
name: r
matcher: { type: Any }
"#;
        let rule: TcpRule = serde_yaml::from_str(yaml).expect("deserialize");
        assert_eq!(rule.response_hex, "");
    }
}
