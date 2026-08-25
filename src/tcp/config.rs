// Schema + chargement du mock TCP brut. Delibrement HORS de
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
//
// AUCUN mode proxy (retire de la premiere version de cette tranche, cf
// `tcp::mod` pour la decision complete) : un relais TCP brut sans matching
// n'ajoute aucune valeur de mock, et fait passer le trafic par un
// intermediaire qui ira de toute facon vers la meme cible relle -- l'
// appelant peut s'y connecter directement, ce que lightMock imposer un
// saut reseau supplementaire sans contrepartie contredit. Chaque
// `TcpService` n'a donc PAS de cible reelle a configurer : uniquement des
// regles de mock.
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
    /// Reponse brute renvoyee au client quand cette regle matche. Encodage
    /// hexadecimal (pas base64) : evite une dependance externe
    /// supplementaire, cf `tcp::hex`.
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
    /// Prefixe attendu, encode en hexadecimal.
    Prefix(String),
    /// Pattern `regex::bytes::Regex` applique aux octets bruts (pas d'UTF-8
    /// garanti).
    Regex(String),
    /// Matche toujours : utile comme regle de repli en fin de liste.
    Any,
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
        let dir =
            std::env::temp_dir().join(format!("lightmock-tcp-cfg-test-{}", fastrand::u64(..)));
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
