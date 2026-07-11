// Support Kafka fonctionnel (feature-gated par "messaging-kafka", desactivee
// par defaut : ce module ne compile meme pas sans la feature, donc zero
// impact sur le binaire/tests par defaut). `KafkaConfig` + parsing env restent
// ici ; le consumer/publisher et le journal des messages sont dans des
// sous-modules dedies. Voir la section "Messaging Kafka" de README.md pour
// le design complet (adaptation du matcher, choix TTL/troncature, JMS non
// supporte, SMTP phase 2).
pub mod consumer;
pub mod matcher;
pub mod message_log;

use message_log::MessageLog;

/// Regroupe l'etat messaging expose a la couche HTTP (AppState) : le journal
/// des messages (toujours present quand la feature est compilee, meme si
/// Kafka est desactive a l'execution — un GET renvoie simplement une liste
/// vide) + le reply_topic/publisher utilises par `POST /api/messaging/simulate`
/// pour emprunter exactement le meme chemin de publication que le vrai
/// consumer (voir consumer::process_message).
#[derive(Clone)]
pub struct MessagingState {
    pub message_log: MessageLog,
    pub reply_topic: Option<String>,
    pub publisher: consumer::Publisher,
}

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

    // Process-wide env vars (KAFKA_*) mutees par ces tests : cargo test lance
    // les fns de test en parallele (threads OS), donc sans serialisation deux
    // tests qui touchent les memes variables peuvent se marcher dessus de
    // facon intermittente (meme pitfall que BACKUP_MAX_COUNT/DATA_PATH/
    // SHOW_RESET_BUTTON, cf src/store/mod.rs et src/auth/mod.rs). Chaque
    // test tient cette variable pour tout son corps.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
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
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        unsafe { std::env::set_var("KAFKA_BROKERS", "broker1:9092, broker2:9092") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.brokers, vec!["broker1:9092", "broker2:9092"]);
        clear_env();
    }

    #[test]
    fn enabled_true_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        unsafe { std::env::set_var("KAFKA_ENABLED", "true") };
        let cfg = KafkaConfig::from_env();
        assert!(cfg.enabled);
        clear_env();
    }

    #[test]
    fn reply_topic_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        unsafe { std::env::set_var("KAFKA_REPLY_TOPIC", "lightmock.replies") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.reply_topic, Some("lightmock.replies".to_string()));
        clear_env();
    }

    #[test]
    fn empty_reply_topic_env_var_is_none() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        unsafe { std::env::set_var("KAFKA_REPLY_TOPIC", "") };
        let cfg = KafkaConfig::from_env();
        assert!(cfg.reply_topic.is_none());
        clear_env();
    }

    #[test]
    fn custom_consumer_group() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        unsafe { std::env::set_var("KAFKA_CONSUMER_GROUP", "my-group") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.consumer_group, "my-group");
        clear_env();
    }
}
