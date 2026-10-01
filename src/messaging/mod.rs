// Kafka messaging, compiled only with the "messaging-kafka" feature: without it, this module and its dependency do
// not exist in the binary. The configuration (`KafkaConfig`, read from the environment) lives here; the consumer
// and publisher, and the message log, have their own modules. docs/kafka-messaging.md describes the behavior.
pub mod consumer;
pub mod matcher;
pub mod message_log;

use message_log::MessageLog;

/// The messaging state the HTTP layer needs: the message log (present whenever the feature is compiled, empty when
/// Kafka is off at run time) and the reply topic and publisher, so that `POST /api/messaging/simulate` publishes
/// exactly as the consumer does (see consumer::process_message).
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
            std::env::var("KAFKA_CONSUMER_GROUP").unwrap_or_else(|_| "mimicway".into());
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

    // KAFKA_* are process-wide and test functions run in parallel: every test that sets them holds this lock for its
    // whole body.
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
        assert_eq!(cfg.consumer_group, "mimicway");
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
        unsafe { std::env::set_var("KAFKA_REPLY_TOPIC", "mimicway.replies") };
        let cfg = KafkaConfig::from_env();
        assert_eq!(cfg.reply_topic, Some("mimicway.replies".to_string()));
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
