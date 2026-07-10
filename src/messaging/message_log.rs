// Journal en memoire des messages Kafka traites (entrants et, le cas echeant,
// les reponses publiees sur reply_topic) — meme principe que RequestLog
// (src/server/request_log.rs, FIFO en memoire), adapte au domaine messaging.
//
// DOUBLE BORNE (voir CLAUDE.md, exigence #9 du sujet messaging) : nombre
// d'entrees (MAX_ENTRIES, comme les 200 de RequestLog) ET age (TTL, defaut
// 24h via MESSAGE_LOG_TTL_MS). Une seule des deux bornes ne suffit pas : un
// flux Kafka peut produire des messages plus vite que le TTL ne les expire
// (la seule age ne bornerait pas la memoire dans ce cas), et inversement un
// flux tres calme laisserait un vieux message dormir des jours sans purge de
// taille (la seule taille ne respecterait pas la contrainte de retention
// courte demandee). Les deux cohabitent : `push()` purge d'abord les entrees
// expirees par age, PUIS applique la borne de taille si necessaire.
//
// Purge opportuniste a l'ecriture (event-driven), jamais de tache de fond —
// meme pattern que purge_expired_protected_backups (src/store/mod.rs) et
// coherent avec le reste du projet (CLAUDE.md #22 : pas de polling/cron).
//
// Troncature du corps : au-dela de MESSAGE_LOG_MAX_BODY_SIZE (defaut 16Ko,
// configurable), le corps stocke est tronque, mais les metadonnees (topic,
// timestamp, taille reelle, statut match/no-match) restent toujours
// completes — seul le contenu potentiellement volumineux est coupe.
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

const MAX_ENTRIES: usize = 500;
const DEFAULT_TTL_MS: u64 = 24 * 60 * 60 * 1000;
const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024;

pub fn ttl_ms() -> u64 {
    std::env::var("MESSAGE_LOG_TTL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TTL_MS)
}

pub fn max_body_size() -> usize {
    std::env::var("MESSAGE_LOG_MAX_BODY_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_SIZE)
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageLogEntry {
    pub timestamp: u64,
    /// "in" (message recu depuis listen_topic) ou "out" (reponse publiee sur reply_topic).
    pub direction: String,
    pub topic: String,
    pub service_name: Option<String>,
    pub rule_matched: Option<String>,
    pub matched: bool,
    pub body_preview: String,
    pub body_truncated: bool,
    pub body_size_bytes: usize,
}

#[derive(Clone)]
pub struct MessageLog {
    entries: Arc<RwLock<VecDeque<MessageLogEntry>>>,
}

impl MessageLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_ENTRIES))),
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn truncate_body(body: &[u8]) -> (String, bool) {
        let max = max_body_size();
        let truncated = body.len() > max;
        let slice = &body[..body.len().min(max)];
        (String::from_utf8_lossy(slice).into_owned(), truncated)
    }

    pub fn push(&self, entry: MessageLogEntry) {
        let mut entries = self.entries.write().unwrap();

        let ttl = ttl_ms();
        let now = Self::now_ms();
        while let Some(front) = entries.front() {
            if now.saturating_sub(front.timestamp) >= ttl {
                entries.pop_front();
            } else {
                break;
            }
        }

        while entries.len() >= MAX_ENTRIES {
            entries.pop_front();
        }

        entries.push_back(entry);
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &self,
        direction: &str,
        topic: &str,
        service_name: Option<&str>,
        rule_matched: Option<&str>,
        matched: bool,
        body: &[u8],
    ) {
        let (body_preview, body_truncated) = Self::truncate_body(body);
        self.push(MessageLogEntry {
            timestamp: Self::now_ms(),
            direction: direction.into(),
            topic: topic.into(),
            service_name: service_name.map(String::from),
            rule_matched: rule_matched.map(String::from),
            matched,
            body_preview,
            body_truncated,
            body_size_bytes: body.len(),
        });
    }

    pub fn record_in(
        &self,
        topic: &str,
        service_name: Option<&str>,
        rule_matched: Option<&str>,
        matched: bool,
        body: &[u8],
    ) {
        self.record("in", topic, service_name, rule_matched, matched, body);
    }

    pub fn record_out(
        &self,
        topic: &str,
        service_name: Option<&str>,
        rule_matched: Option<&str>,
        body: &[u8],
    ) {
        self.record("out", topic, service_name, rule_matched, true, body);
    }

    pub fn recent(&self, limit: usize) -> Vec<MessageLogEntry> {
        let entries = self.entries.read().unwrap();
        entries.iter().rev().take(limit).cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }
}

impl Default for MessageLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn ttl_default_is_24h() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("MESSAGE_LOG_TTL_MS") };
        assert_eq!(ttl_ms(), 24 * 60 * 60 * 1000);
    }

    #[test]
    fn ttl_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MESSAGE_LOG_TTL_MS", "1000") };
        assert_eq!(ttl_ms(), 1000);
        unsafe { std::env::remove_var("MESSAGE_LOG_TTL_MS") };
    }

    #[test]
    fn max_body_size_default_is_16kb() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("MESSAGE_LOG_MAX_BODY_SIZE") };
        assert_eq!(max_body_size(), 16 * 1024);
    }

    #[test]
    fn max_body_size_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MESSAGE_LOG_MAX_BODY_SIZE", "10") };
        assert_eq!(max_body_size(), 10);
        unsafe { std::env::remove_var("MESSAGE_LOG_MAX_BODY_SIZE") };
    }

    #[test]
    fn record_in_matched_entry() {
        // Meme si ce test ne touche pas lui-meme MESSAGE_LOG_MAX_BODY_SIZE,
        // record_in() lit max_body_size() en interne : sans le meme mutex, un
        // test concurrent qui positionne temporairement cette variable peut
        // fausser la troncature observee ici (race inter-threads sur un env
        // var process-wide, meme pitfall que documente en tete de module).
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = MessageLog::new();
        log.record_in("orders.in", Some("svc-a"), Some("rule-1"), true, b"hello");
        let entries = log.recent(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].direction, "in");
        assert_eq!(entries[0].topic, "orders.in");
        assert_eq!(entries[0].service_name.as_deref(), Some("svc-a"));
        assert_eq!(entries[0].rule_matched.as_deref(), Some("rule-1"));
        assert!(entries[0].matched);
        assert_eq!(entries[0].body_preview, "hello");
        assert!(!entries[0].body_truncated);
        assert_eq!(entries[0].body_size_bytes, 5);
    }

    #[test]
    fn record_in_no_match_entry() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = MessageLog::new();
        log.record_in("orders.in", None, None, false, b"unmatched");
        let entries = log.recent(10);
        assert!(!entries[0].matched);
        assert!(entries[0].service_name.is_none());
        assert!(entries[0].rule_matched.is_none());
    }

    #[test]
    fn record_out_entry() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = MessageLog::new();
        log.record_out("orders.reply", Some("svc-a"), Some("rule-1"), b"response body");
        let entries = log.recent(10);
        assert_eq!(entries[0].direction, "out");
        assert_eq!(entries[0].topic, "orders.reply");
    }

    #[test]
    fn body_truncated_beyond_max_size() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MESSAGE_LOG_MAX_BODY_SIZE", "5") };
        let log = MessageLog::new();
        log.record_in("t", None, None, false, b"0123456789");
        let entries = log.recent(10);
        assert!(entries[0].body_truncated);
        assert_eq!(entries[0].body_preview, "01234");
        assert_eq!(entries[0].body_size_bytes, 10, "real size must be preserved even when body is truncated");
        unsafe { std::env::remove_var("MESSAGE_LOG_MAX_BODY_SIZE") };
    }

    #[test]
    fn body_not_truncated_under_max_size() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MESSAGE_LOG_MAX_BODY_SIZE", "100") };
        let log = MessageLog::new();
        log.record_in("t", None, None, false, b"short");
        let entries = log.recent(10);
        assert!(!entries[0].body_truncated);
        unsafe { std::env::remove_var("MESSAGE_LOG_MAX_BODY_SIZE") };
    }

    #[test]
    fn entry_count_bounded_by_max_entries() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = MessageLog::new();
        for i in 0..(MAX_ENTRIES + 50) {
            log.record_in("t", None, None, false, format!("msg-{i}").as_bytes());
        }
        assert_eq!(log.len(), MAX_ENTRIES, "entry count must never exceed MAX_ENTRIES");
        let entries = log.recent(1);
        assert_eq!(entries[0].body_preview, format!("msg-{}", MAX_ENTRIES + 49));
    }

    #[test]
    fn expired_entries_purged_on_next_write() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MESSAGE_LOG_TTL_MS", "50") };
        let log = MessageLog::new();
        log.push(MessageLogEntry {
            timestamp: MessageLog::now_ms() - 1000,
            direction: "in".into(),
            topic: "t".into(),
            service_name: None,
            rule_matched: None,
            matched: false,
            body_preview: "old".into(),
            body_truncated: false,
            body_size_bytes: 3,
        });
        assert_eq!(log.len(), 1);

        // Any subsequent write is the purge trigger (event-driven, no timer).
        log.record_in("t", None, None, false, b"new");
        assert_eq!(log.len(), 1, "expired entry must be purged, only the fresh one remains");
        assert_eq!(log.recent(1)[0].body_preview, "new");
        unsafe { std::env::remove_var("MESSAGE_LOG_TTL_MS") };
    }

    #[test]
    fn fresh_entries_survive_purge() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("MESSAGE_LOG_TTL_MS", "60000") };
        let log = MessageLog::new();
        log.record_in("t", None, None, false, b"a");
        log.record_in("t", None, None, false, b"b");
        assert_eq!(log.len(), 2);
        unsafe { std::env::remove_var("MESSAGE_LOG_TTL_MS") };
    }

    #[test]
    fn recent_returns_most_recent_first() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = MessageLog::new();
        log.record_in("t", None, None, false, b"first");
        log.record_in("t", None, None, false, b"second");
        let entries = log.recent(10);
        assert_eq!(entries[0].body_preview, "second");
        assert_eq!(entries[1].body_preview, "first");
    }
}
