// Cache du statut de disponibilite de la cible reelle (real_target_url) d'un
// service. Le statut lui-meme (PingStatus) est produit par ProxyClient::ping()
// (src/engine/proxy.rs) et n'est jamais persiste dans Service/YAML (cf
// CLAUDE.md #16 : pas de retrocompat serde sur les champs obligatoires) — c'est
// un etat transitoire en memoire, tenu par nom de service.
//
// Decision : test a la demande (bouton UI -> POST /api/services/:name/ping) +
// cache TTL court cote serveur, PAS de tache de fond/cron. Un clic repete dans
// la fenetre PING_TTL_MS renvoie le resultat en cache (lookup HashMap O(1))
// au lieu de relancer une requete reseau — c'est le compromis le plus econome
// (green IT, pas de polling permanent, cout proportionnel a l'usage reel).
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
        cache.set("svc-a", PingStatus { reachable: true, checked_at: now_ms(), error: None });
        let got = cache.get_fresh("svc-a", PING_TTL_MS);
        assert!(got.is_some());
        assert!(got.unwrap().reachable);
    }

    #[test]
    fn expired_entry_returns_none() {
        let cache = PingCache::new();
        cache.set("svc-a", PingStatus { reachable: true, checked_at: now_ms() - 500, error: None });
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
        cache.set("svc-a", PingStatus { reachable: false, checked_at: now_ms(), error: Some("boom".into()) });
        cache.set("svc-a", PingStatus { reachable: true, checked_at: now_ms(), error: None });
        let got = cache.get_fresh("svc-a", PING_TTL_MS).unwrap();
        assert!(got.reachable);
        assert!(got.error.is_none());
    }
}
