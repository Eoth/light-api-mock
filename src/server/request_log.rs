use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

const MAX_ENTRIES: usize = 200;

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

    pub fn log_mock(&self, service: &str, method: &str, path: &str, rule: &str, status: u16) {
        self.push(LogEntry {
            timestamp: Self::now_ms(),
            service_name: service.into(),
            method: method.into(),
            path: path.into(),
            mode: "mock".into(),
            rule_matched: Some(rule.into()),
            target_url: None,
            status,
        });
    }

    pub fn log_proxy(&self, service: &str, method: &str, path: &str, target: &str, status: u16) {
        self.push(LogEntry {
            timestamp: Self::now_ms(),
            service_name: service.into(),
            method: method.into(),
            path: path.into(),
            mode: "proxy".into(),
            rule_matched: None,
            target_url: Some(target.into()),
            status,
        });
    }

    pub fn log_no_rule(&self, service: &str, method: &str, path: &str) {
        self.push(LogEntry {
            timestamp: Self::now_ms(),
            service_name: service.into(),
            method: method.into(),
            path: path.into(),
            mode: "no-rule".into(),
            rule_matched: None,
            target_url: None,
            status: 404,
        });
    }
}
