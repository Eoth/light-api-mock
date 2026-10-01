// Recent results of the availability check of each service's real target (ProxyClient::ping, a TCP connection
// only), kept in memory per service and never saved with the configuration.
//
// The check runs when a user asks for it, never in the background: a repeated click within PING_TTL_MS gets the
// cached result instead of a new connection, so the network cost follows actual use.
use crate::engine::PingStatus;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub const PING_TTL_MS: u64 = 120_000;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Clone)]
pub struct PingCache {
    entries: Arc<RwLock<HashMap<String, PingStatus>>>,
}

impl Default for PingCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PingCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_fresh(&self, service_name: &str, ttl_ms: u64) -> Option<PingStatus> {
        let entries = self.entries.read().unwrap();
        let status = entries.get(service_name)?;
        if now_ms().saturating_sub(status.checked_at) < ttl_ms {
            Some(status.clone())
        } else {
            None
        }
    }

    pub fn set(&self, service_name: &str, status: PingStatus) {
        let mut entries = self.entries.write().unwrap();
        entries.insert(service_name.to_string(), status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_entry_is_returned() {
        let cache = PingCache::new();
        cache.set(
            "svc-a",
            PingStatus {
                reachable: true,
                checked_at: now_ms(),
                error: None,
            },
        );
        let got = cache.get_fresh("svc-a", PING_TTL_MS);
        assert!(got.is_some());
        assert!(got.unwrap().reachable);
    }

    #[test]
    fn expired_entry_returns_none() {
        let cache = PingCache::new();
        cache.set(
            "svc-a",
            PingStatus {
                reachable: true,
                checked_at: now_ms() - 500,
                error: None,
            },
        );
        assert!(cache.get_fresh("svc-a", 100).is_none());
    }

    #[test]
    fn unknown_service_returns_none() {
        let cache = PingCache::new();
        assert!(cache.get_fresh("unknown", PING_TTL_MS).is_none());
    }

    #[test]
    fn set_overwrites_previous_entry() {
        let cache = PingCache::new();
        cache.set(
            "svc-a",
            PingStatus {
                reachable: false,
                checked_at: now_ms(),
                error: Some("boom".into()),
            },
        );
        cache.set(
            "svc-a",
            PingStatus {
                reachable: true,
                checked_at: now_ms(),
                error: None,
            },
        );
        let got = cache.get_fresh("svc-a", PING_TTL_MS).unwrap();
        assert!(got.reachable);
        assert!(got.error.is_none());
    }
}
