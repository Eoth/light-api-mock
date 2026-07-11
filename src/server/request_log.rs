use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

const MAX_ENTRIES: usize = 200;
const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024;

/// Taille maximale (en octets) du corps retenu dans `CapturedRequest::body`.
/// Meme idiome que `messaging::message_log::max_body_size` (env var, defaut
/// 16 Ko) : les 200 entrees de `RequestLog` pourraient sinon retenir jusqu'a
/// plusieurs Mo de corps chacune (le corps est deja entierement bufferise en
/// amont pour le matching, cf `intercept.rs`), ce qui grillerait la memoire
/// du pod (PVC/limite 64Mi) sans utilite pour le testeur de regle.
pub fn max_body_size() -> usize {
    std::env::var("REQUEST_LOG_MAX_BODY_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_SIZE)
}

/// Detail d'une requete HTTP reellement recue, retenu pour permettre au
/// testeur de regle (UI) de rejouer une regle EN COURS D'EDITION contre du
/// trafic reel, en lecture seule. `path_params` est le sous-ensemble
/// SERVICE-LEVEL uniquement (avant fusion avec les parametres du `sub_path`
/// de la regle qui avait matche a la capture) : le testeur recalcule les
/// parametres du `sub_path` pour le brouillon de regle en cours d'edition,
/// qui peut differer de la regle qui avait matche a l'origine.
///
/// N'existe QUE pour les requetes qui passent par `handle_service`
/// (mock, no-rule, proxy niveau regle) : ces requetes bufferisent deja
/// integralement le corps pour le matching (`RequestData`), donc retenir ce
/// detail ne cree AUCUNE nouvelle capture de trafic — on ne fait que
/// conserver ce qui est deja en memoire a cet instant. Reste `None` pour le
/// proxy niveau service (`is_mocked=false`) : ce chemin est volontairement
/// streame sans buffering ("Proxy streaming"), et le construire
/// la introduirait un `to_bytes()` qui n'existe pas aujourd'hui.
#[derive(Debug, Clone, Serialize)]
pub struct CapturedRequest {
    pub remaining_path: String,
    pub path_params: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_truncated: bool,
    pub content_type: Option<String>,
}

impl CapturedRequest {
    pub fn from_request_data(req: &crate::engine::matcher::RequestData) -> Self {
        let max = max_body_size();
        let truncated = req.body.len() > max;
        let slice = &req.body[..req.body.len().min(max)];
        Self {
            remaining_path: req.remaining_path.clone(),
            path_params: req.path_params.clone(),
            query_params: req.query_params.clone(),
            headers: req.headers.clone(),
            body: String::from_utf8_lossy(slice).into_owned(),
            body_truncated: truncated,
            content_type: req.content_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub service_name: String,
    pub method: String,
    pub path: String,
    pub mode: String,
    pub rule_matched: Option<String>,
    pub target_url: Option<String>,
    pub status: u16,
    pub captured: Option<CapturedRequest>,
}

#[derive(Clone)]
pub struct RequestLog {
    entries: Arc<RwLock<VecDeque<LogEntry>>>,
}

impl RequestLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_ENTRIES))),
        }
    }

    pub fn push(&self, entry: LogEntry) {
        let mut entries = self.entries.write().unwrap();
        if entries.len() >= MAX_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        let entries = self.entries.read().unwrap();
        entries.iter().rev().take(limit).cloned().collect()
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn log_mock(
        &self,
        service: &str,
        method: &str,
        path: &str,
        rule: &str,
        status: u16,
        captured: Option<CapturedRequest>,
    ) {
        self.push(LogEntry {
            timestamp: Self::now_ms(),
            service_name: service.into(),
            method: method.into(),
            path: path.into(),
            mode: "mock".into(),
            rule_matched: Some(rule.into()),
            target_url: None,
            status,
            captured,
        });
    }

    pub fn log_proxy(
        &self,
        service: &str,
        method: &str,
        path: &str,
        target: &str,
        status: u16,
        captured: Option<CapturedRequest>,
    ) {
        self.push(LogEntry {
            timestamp: Self::now_ms(),
            service_name: service.into(),
            method: method.into(),
            path: path.into(),
            mode: "proxy".into(),
            rule_matched: None,
            target_url: Some(target.into()),
            status,
            captured,
        });
    }

    pub fn log_no_rule(
        &self,
        service: &str,
        method: &str,
        path: &str,
        captured: Option<CapturedRequest>,
    ) {
        self.push(LogEntry {
            timestamp: Self::now_ms(),
            service_name: service.into(),
            method: method.into(),
            path: path.into(),
            mode: "no-rule".into(),
            rule_matched: None,
            target_url: None,
            status: 404,
            captured,
        });
    }
}

impl Default for RequestLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::matcher::RequestData;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn req_data(body: &[u8]) -> RequestData {
        RequestData {
            query_params: HashMap::from([("id".to_string(), "42".to_string())]),
            headers: HashMap::from([("x-env".to_string(), "prod".to_string())]),
            body: body.to_vec(),
            content_type: Some("application/json".into()),
            path_params: HashMap::from([("svcParam".to_string(), "abc".to_string())]),
            method: "GET".into(),
            remaining_path: "/orders/1".into(),
        }
    }

    #[test]
    fn max_body_size_default_is_16kb() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("REQUEST_LOG_MAX_BODY_SIZE") };
        assert_eq!(max_body_size(), 16 * 1024);
    }

    #[test]
    fn max_body_size_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("REQUEST_LOG_MAX_BODY_SIZE", "10") };
        assert_eq!(max_body_size(), 10);
        unsafe { std::env::remove_var("REQUEST_LOG_MAX_BODY_SIZE") };
    }

    #[test]
    fn captured_request_preserves_full_detail_under_limit() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("REQUEST_LOG_MAX_BODY_SIZE") };
        let data = req_data(b"{\"a\":1}");
        let captured = CapturedRequest::from_request_data(&data);
        assert_eq!(captured.body, "{\"a\":1}");
        assert!(!captured.body_truncated);
        assert_eq!(captured.query_params.get("id").unwrap(), "42");
        assert_eq!(captured.path_params.get("svcParam").unwrap(), "abc");
        assert_eq!(captured.remaining_path, "/orders/1");
        assert_eq!(captured.content_type.as_deref(), Some("application/json"));
    }

    #[test]
    fn captured_request_truncates_body_beyond_max_size() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("REQUEST_LOG_MAX_BODY_SIZE", "5") };
        let data = req_data(b"0123456789");
        let captured = CapturedRequest::from_request_data(&data);
        assert!(captured.body_truncated);
        assert_eq!(captured.body, "01234");
        unsafe { std::env::remove_var("REQUEST_LOG_MAX_BODY_SIZE") };
    }

    #[test]
    fn log_mock_stores_captured_detail() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = RequestLog::new();
        let captured = CapturedRequest::from_request_data(&req_data(b"body"));
        log.log_mock("svc", "GET", "/svc/orders/1", "rule-1", 200, Some(captured));
        let entries = log.recent(1);
        assert!(entries[0].captured.is_some());
        assert_eq!(
            entries[0].captured.as_ref().unwrap().remaining_path,
            "/orders/1"
        );
    }

    #[test]
    fn log_proxy_service_level_has_no_captured_detail() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = RequestLog::new();
        log.log_proxy("svc", "GET", "/svc/orders/1", "http://backend/orders/1", 200, None);
        let entries = log.recent(1);
        assert!(entries[0].captured.is_none());
    }

    #[test]
    fn log_no_rule_stores_captured_detail() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = RequestLog::new();
        let captured = CapturedRequest::from_request_data(&req_data(b""));
        log.log_no_rule("svc", "GET", "/svc/unknown", Some(captured));
        let entries = log.recent(1);
        assert!(entries[0].captured.is_some());
    }

    #[test]
    fn entry_count_bounded_by_max_entries() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = RequestLog::new();
        for i in 0..(MAX_ENTRIES + 20) {
            log.log_no_rule("svc", "GET", &format!("/svc/{i}"), None);
        }
        assert_eq!(log.recent(usize::MAX).len(), MAX_ENTRIES);
        let entries = log.recent(1);
        assert_eq!(entries[0].path, format!("/svc/{}", MAX_ENTRIES + 19));
    }

    #[test]
    fn recent_returns_most_recent_first() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let log = RequestLog::new();
        log.log_no_rule("svc", "GET", "/svc/first", None);
        log.log_no_rule("svc", "GET", "/svc/second", None);
        let entries = log.recent(10);
        assert_eq!(entries[0].path, "/svc/second");
        assert_eq!(entries[1].path, "/svc/first");
    }
}
