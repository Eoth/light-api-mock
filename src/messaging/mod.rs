// Cadrage MOM/Kafka (etude de faisabilite, feature-gated par "messaging-kafka",
// desactivee par defaut : ce module ne compile meme pas sans la feature, donc
// zero impact sur le binaire/tests par defaut). Contient UNIQUEMENT le modele
// de configuration et son parsing depuis l'environnement — aucune dependance
// reseau/Kafka (rdkafka) n'est ajoutee dans cette passe, volontairement, pour
// eviter d'alourdir le binaire/la chaine de build tant que le besoin n'est
// pas confirme. Voir CLAUDE.md section "Support MOM / Messaging" pour le
// design technique complet (architecture consumer/matching/publish prevue,
// impact taille binaire, JMS non supporte, SMTP phase 2).
#[derive(Debug, Clone, PartialEq)]
pub struct KafkaConfig {
    pub enabled: bool,
    pub brokers: Vec<String>,
    pub consumer_group: String,
    pub listen_topic: String,
    pub reply_topic: Option<String>,
}

impl KafkaConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("KAFKA_ENABLED")
            .unwrap_or_else(|_| "false".into())
            .eq_ignore_ascii_case("true");

        let brokers: Vec<String> = std::env::var("KAFKA_BROKERS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let consumer_group =
            std::env::var("KAFKA_CONSUMER_GROUP").unwrap_or_else(|_| "lightmock".into());
        let listen_topic = std::env::var("KAFKA_LISTEN_TOPIC").unwrap_or_default();
        let reply_topic = std::env::var("KAFKA_REPLY_TOPIC")
            .ok()
            .filter(|s| !s.is_empty());

        Self {
            enabled,
            brokers,
            consumer_group,
            listen_topic,
            reply_topic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_env() {
        unsafe {
            std::env::remove_var("KAFKA_ENABLED");
            std::env::remove_var("KAFKA_BROKERS");
            std::env::remove_var("KAFKA_CONSUMER_GROUP");
            std::env::remove_var("KAFKA_LISTEN_TOPIC");
            std::env::remove_var("KAFKA_REPLY_TOPIC");
        }
    }

    #[test]
    fn defaults_disabled_with_empty_brokers() {
        clear_env();
        let cfg = KafkaConfig::from_env();
        assert!(!cfg.enabled);
        assert!(cfg.brokers.is_empty());
        assert_eq!(cfg.consumer_group, "lightmock");
        assert_eq!(cfg.listen_topic, "");
        assert!(cfg.reply_topic.is_none());
    }

    #[test]
    fn parses_broker_list_from_env() {
        clear_env();
        unsafe { std::env::set_var("KAFKA_BROKERS", "broker1:9092, broker2:9092") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.brokers, vec!["broker1:9092", "broker2:9092"]);
        clear_env();
    }

    #[test]
    fn enabled_true_from_env() {
        clear_env();
        unsafe { std::env::set_var("KAFKA_ENABLED", "true") };
        let cfg = KafkaConfig::from_env();
        assert!(cfg.enabled);
        clear_env();
    }

    #[test]
    fn reply_topic_from_env() {
        clear_env();
        unsafe { std::env::set_var("KAFKA_REPLY_TOPIC", "lightmock.replies") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.reply_topic, Some("lightmock.replies".to_string()));
        clear_env();
    }

    #[test]
    fn empty_reply_topic_env_var_is_none() {
        clear_env();
        unsafe { std::env::set_var("KAFKA_REPLY_TOPIC", "") };
        let cfg = KafkaConfig::from_env();
        assert!(cfg.reply_topic.is_none());
        clear_env();
    }

    #[test]
    fn custom_consumer_group() {
        clear_env();
        unsafe { std::env::set_var("KAFKA_CONSUMER_GROUP", "my-group") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.consumer_group, "my-group");
        clear_env();
    }
}
