// Observation de trafic proxy niveau service (is_mocked=false), pour armer
// une future suggestion automatique de regles de mock (cf commentaire en tete
// de `intercept.rs::do_proxy` pour le branchement complet). Deux etats
// distincts, tous deux ephemeres (jamais persistes, jamais dans MockConfig/
// YAML) :
//
// - `ObservationToggle` : quel service l'utilisateur observe ACTIVEMENT en ce
//   moment (active/desactive explicitement via l'API, PAS automatique — le
//   proxy niveau service reste 100% streame sans aucun cout ajoute tant que
//   personne ne l'a active pour ce service precis). Meme genre d'etat
//   transitoire que `ping::PingCache`, jamais dans Service/YAML.
// - `ObservationStore` : les echanges (requete + reponse cible) captures
//   pendant qu'un service est observe, groupes par (service, method,
//   sub_path litteral) pour permettre une future correlation/detection de
//   variance. Borne en memoire sur deux axes (nombre de cles ET
//   d'echantillons par cle) — jamais de croissance non bornee.
//
// Ne construit PAS encore de suggestion de regle : ce module est le
// prerequis (capture + stockage borne), pas l'algorithme de correlation.
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};

const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024;
const DEFAULT_MAX_BUFFER_SIZE: usize = 10 * 1024 * 1024;
const DEFAULT_SAMPLES_PER_KEY: usize = 8;
const DEFAULT_MAX_KEYS: usize = 200;

/// Taille max (octets) du corps RETENU dans un `ObservedExchange` (requete et
/// reponse tronquees independamment a cette limite). Meme idiome que
/// `request_log::max_body_size` / `message_log::max_body_size`.
pub fn max_body_size() -> usize {
    std::env::var("TRAFFIC_OBSERVATION_MAX_BODY_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_SIZE)
}

/// Taille max (octets) qu'un corps (requete ou reponse) peut atteindre pour
/// etre EFFECTIVEMENT capture (via Content-Length declare) : au-dela, ou en
/// l'absence de Content-Length, l'echange n'est pas mis en buffer pour la
/// capture et repasse par le chemin de streaming habituel, jamais observe.
/// Meme plafond que le cap deja applique cote requete mock (`intercept.rs`,
/// `axum::body::to_bytes(..., 10 * 1024 * 1024)`), pour cohérence.
pub fn max_buffer_size() -> usize {
    std::env::var("TRAFFIC_OBSERVATION_MAX_BUFFER_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_BUFFER_SIZE)
}

/// Nombre max d'echanges retenus par cle (service, method, sub_path). Au-dela,
/// le plus ancien est evince (FIFO), meme principe que `RequestLog`.
pub fn samples_per_key() -> usize {
    std::env::var("TRAFFIC_OBSERVATION_SAMPLES_PER_KEY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SAMPLES_PER_KEY)
}

/// Nombre max de cles (endpoints distincts) suivies simultanement, tous
/// services observes confondus. Au-dela, la cle la moins recemment mise a
/// jour est evincee pour faire de la place a une nouvelle cle.
pub fn max_keys() -> usize {
    std::env::var("TRAFFIC_OBSERVATION_MAX_KEYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_KEYS)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn truncate_body(bytes: &[u8]) -> (String, bool) {
    let max = max_body_size();
    let truncated = bytes.len() > max;
    let slice = &bytes[..bytes.len().min(max)];
    (String::from_utf8_lossy(slice).into_owned(), truncated)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ObservationKey {
    pub group_name: Option<String>,
    pub service_name: String,
    pub method: String,
    /// Chemin restant litteral relatif au service (PAS un pattern de regle :
    /// le proxy niveau service n'a pas de `sub_path` templatise, seulement le
    /// chemin reel de chaque appel). Deux chemins qui ne different que par un
    /// segment variable (ex. `/orders/1` vs `/orders/2`) forment aujourd'hui
    /// DEUX cles distinctes — pas d'inference de path param en v1, cf note
    /// de conception.
    pub sub_path: String,
}

/// Un echange requete/reponse REELLEMENT capture (les deux cotes bufferises
/// avec succes sous `max_buffer_size()`). Un exchange partiel (reponse trop
/// grosse/streamee, Content-Length absent...) n'est jamais pousse dans le
/// store — voir `ProxyClient::forward_with_capture`.
#[derive(Debug, Clone, Serialize)]
pub struct ObservedExchange {
    pub timestamp: u64,
    pub request_query_params: HashMap<String, String>,
    pub request_headers: HashMap<String, String>,
    pub request_body: String,
    pub request_body_truncated: bool,
    pub request_content_type: Option<String>,
    pub response_status: u16,
    pub response_headers: HashMap<String, String>,
    pub response_body: String,
    pub response_body_truncated: bool,
    pub response_content_type: Option<String>,
}

impl ObservedExchange {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_query_params: HashMap<String, String>,
        request_headers: HashMap<String, String>,
        request_body: &[u8],
        request_content_type: Option<String>,
        response_status: u16,
        response_headers: HashMap<String, String>,
        response_body: &[u8],
        response_content_type: Option<String>,
    ) -> Self {
        let (req_body, req_truncated) = truncate_body(request_body);
        let (resp_body, resp_truncated) = truncate_body(response_body);
        Self {
            timestamp: now_ms(),
            request_query_params,
            request_headers,
            request_body: req_body,
            request_body_truncated: req_truncated,
            request_content_type,
            response_status,
            response_headers,
            response_body: resp_body,
            response_body_truncated: resp_truncated,
            response_content_type,
        }
    }
}

#[derive(Default)]
struct ObservationInner {
    buckets: HashMap<ObservationKey, VecDeque<ObservedExchange>>,
    // Ordre de derniere mise a jour des cles (front = la moins recente),
    // pour eviction quand `max_keys()` est atteint. Une cle deja connue est
    // deplacee en fin de liste a chaque `record()`.
    key_order: VecDeque<ObservationKey>,
}

/// Buffer borne des echanges observes, indexe par (service, method,
/// sub_path). Cf commentaire de module pour les deux axes de bornage.
#[derive(Clone)]
pub struct ObservationStore {
    inner: Arc<RwLock<ObservationInner>>,
}

impl ObservationStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ObservationInner::default())),
        }
    }

    pub fn record(&self, key: ObservationKey, exchange: ObservedExchange) {
        let mut inner = self.inner.write().unwrap();
        let is_new_key = !inner.buckets.contains_key(&key);

        if is_new_key
            && inner.buckets.len() >= max_keys()
            && let Some(evicted) = inner.key_order.pop_front()
        {
            inner.buckets.remove(&evicted);
        }

        if let Some(pos) = inner.key_order.iter().position(|k| k == &key) {
            inner.key_order.remove(pos);
        }
        inner.key_order.push_back(key.clone());

        let bucket = inner.buckets.entry(key).or_default();
        let cap = samples_per_key();
        if bucket.len() >= cap {
            bucket.pop_front();
        }
        bucket.push_back(exchange);
    }

    /// Copie des echanges retenus pour une cle donnee, plus ancien en
    /// premier. Vide si la cle est inconnue.
    pub fn observations(&self, key: &ObservationKey) -> Vec<ObservedExchange> {
        let inner = self.inner.read().unwrap();
        inner
            .buckets
            .get(key)
            .map(|b| b.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Nombre de cles distinctes suivies actuellement (diagnostic/tests).
    pub fn key_count(&self) -> usize {
        self.inner.read().unwrap().buckets.len()
    }

    /// Toutes les cles (endpoints distincts) suivies pour un service donne,
    /// utilisee par le calcul de suggestions (`server::suggestion`) pour
    /// savoir quels (method, sub_path) examiner sans que l'appelant ait deja
    /// besoin de les connaitre a l'avance.
    pub fn keys_for_service(&self, group: Option<&str>, service_name: &str) -> Vec<ObservationKey> {
        let inner = self.inner.read().unwrap();
        inner
            .buckets
            .keys()
            .filter(|k| k.group_name.as_deref() == group && k.service_name == service_name)
            .cloned()
            .collect()
    }
}

impl Default for ObservationStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Identifiant (group_name, name) d'un service, coherent avec
/// `service_matches` (server/api.rs).
type ServiceKey = (Option<String>, String);

/// Quels services sont actuellement observes, active/desactive explicitement
/// par l'utilisateur (jamais automatique).
#[derive(Clone)]
pub struct ObservationToggle {
    active: Arc<RwLock<HashSet<ServiceKey>>>,
}

impl ObservationToggle {
    pub fn new() -> Self {
        Self {
            active: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    fn key(group: Option<&str>, name: &str) -> ServiceKey {
        (group.map(|g| g.to_string()), name.to_string())
    }

    pub fn enable(&self, group: Option<&str>, name: &str) {
        self.active.write().unwrap().insert(Self::key(group, name));
    }

    pub fn disable(&self, group: Option<&str>, name: &str) {
        self.active.write().unwrap().remove(&Self::key(group, name));
    }

    /// Verifie le statut d'observation SANS allouer : appelee sur le chemin
    /// chaud du proxy niveau service (`handle_service`), a chaque requete,
    /// meme quand l'observation est desactivee. Balayage lineaire plutot
    /// qu'un `HashSet::contains` (qui exigerait de construire une cle
    /// `(Option<String>, String)` a chaque appel) — negligeable en pratique,
    /// le nombre de services observes simultanement restant du ressort de
    /// choix explicites de l'utilisateur (quelques unites au plus).
    pub fn is_enabled(&self, group: Option<&str>, name: &str) -> bool {
        self.active
            .read()
            .unwrap()
            .iter()
            .any(|(g, n)| g.as_deref() == group && n == name)
    }

    /// Liste des services actuellement observes (diagnostic/API de statut).
    pub fn active_services(&self) -> Vec<ServiceKey> {
        self.active.read().unwrap().iter().cloned().collect()
    }

    /// Utilisee uniquement par `reset_config` (suppression de TOUS les
    /// services) : sans ca, un service recree apres coup heriterait
    /// silencieusement du statut "observe" d'un service disparu.
    pub fn clear_all(&self) {
        self.active.write().unwrap().clear();
    }
}

impl Default for ObservationToggle {
    fn default() -> Self {
        Self::new()
    }
}

/// Regroupe les deux etats d'observation dans un seul champ `AppState`, meme
/// convention que `messaging::MessagingState`.
#[derive(Clone)]
pub struct ObservationState {
    pub toggle: ObservationToggle,
    pub store: ObservationStore,
}

impl ObservationState {
    pub fn new() -> Self {
        Self {
            toggle: ObservationToggle::new(),
            store: ObservationStore::new(),
        }
    }
}

impl Default for ObservationState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn sample_key(path: &str) -> ObservationKey {
        ObservationKey {
            group_name: None,
            service_name: "svc".into(),
            method: "GET".into(),
            sub_path: path.into(),
        }
    }

    fn sample_exchange(body: &[u8]) -> ObservedExchange {
        ObservedExchange::new(
            HashMap::new(),
            HashMap::new(),
            body,
            Some("application/json".into()),
            200,
            HashMap::new(),
            body,
            Some("application/json".into()),
        )
    }

    #[test]
    fn toggle_starts_disabled() {
        let toggle = ObservationToggle::new();
        assert!(!toggle.is_enabled(None, "svc"));
    }

    #[test]
    fn toggle_enable_disable_round_trip() {
        let toggle = ObservationToggle::new();
        toggle.enable(None, "svc");
        assert!(toggle.is_enabled(None, "svc"));
        toggle.disable(None, "svc");
        assert!(!toggle.is_enabled(None, "svc"));
    }

    #[test]
    fn toggle_distinguishes_group_scope() {
        let toggle = ObservationToggle::new();
        toggle.enable(Some("teamA"), "svc");
        assert!(toggle.is_enabled(Some("teamA"), "svc"));
        assert!(!toggle.is_enabled(None, "svc"));
        assert!(!toggle.is_enabled(Some("teamB"), "svc"));
    }

    #[test]
    fn toggle_active_services_lists_enabled_only() {
        let toggle = ObservationToggle::new();
        toggle.enable(None, "svc-a");
        toggle.enable(Some("g"), "svc-b");
        let mut active = toggle.active_services();
        active.sort();
        assert_eq!(
            active,
            vec![
                (None, "svc-a".to_string()),
                (Some("g".to_string()), "svc-b".to_string())
            ]
        );
    }

    #[test]
    fn toggle_clear_all_disables_every_service() {
        let toggle = ObservationToggle::new();
        toggle.enable(None, "svc-a");
        toggle.enable(Some("g"), "svc-b");
        toggle.clear_all();
        assert!(toggle.active_services().is_empty());
        assert!(!toggle.is_enabled(None, "svc-a"));
        assert!(!toggle.is_enabled(Some("g"), "svc-b"));
    }

    #[test]
    fn store_records_and_returns_observations_in_order() {
        // sample_exchange() lit max_body_size() (env partagee entre tests) :
        // meme garde que les tests qui la fixent explicitement, sinon course
        // possible avec exchange_truncates_bodies_beyond_max_size en parallele.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let store = ObservationStore::new();
        let key = sample_key("/orders/1");
        store.record(key.clone(), sample_exchange(b"{\"a\":1}"));
        store.record(key.clone(), sample_exchange(b"{\"a\":2}"));
        let observed = store.observations(&key);
        assert_eq!(observed.len(), 2);
        assert_eq!(observed[0].response_body, "{\"a\":1}");
        assert_eq!(observed[1].response_body, "{\"a\":2}");
    }

    #[test]
    fn store_unknown_key_returns_empty() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let store = ObservationStore::new();
        assert!(store.observations(&sample_key("/unknown")).is_empty());
    }

    #[test]
    fn store_bounds_samples_per_key() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("TRAFFIC_OBSERVATION_SAMPLES_PER_KEY", "2") };
        let store = ObservationStore::new();
        let key = sample_key("/orders/1");
        store.record(key.clone(), sample_exchange(b"1"));
        store.record(key.clone(), sample_exchange(b"2"));
        store.record(key.clone(), sample_exchange(b"3"));
        let observed = store.observations(&key);
        assert_eq!(observed.len(), 2, "borne au sample cap, plus ancien evince");
        assert_eq!(observed[0].response_body, "2");
        assert_eq!(observed[1].response_body, "3");
        unsafe { std::env::remove_var("TRAFFIC_OBSERVATION_SAMPLES_PER_KEY") };
    }

    #[test]
    fn store_bounds_distinct_keys() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("TRAFFIC_OBSERVATION_MAX_KEYS", "2") };
        let store = ObservationStore::new();
        store.record(sample_key("/a"), sample_exchange(b"a"));
        store.record(sample_key("/b"), sample_exchange(b"b"));
        store.record(sample_key("/c"), sample_exchange(b"c"));
        assert_eq!(store.key_count(), 2, "borne au nombre de cles max");
        assert!(
            store.observations(&sample_key("/a")).is_empty(),
            "la cle la moins recemment mise a jour doit etre evincee"
        );
        assert!(!store.observations(&sample_key("/c")).is_empty());
        unsafe { std::env::remove_var("TRAFFIC_OBSERVATION_MAX_KEYS") };
    }

    #[test]
    fn store_touching_existing_key_protects_it_from_eviction() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("TRAFFIC_OBSERVATION_MAX_KEYS", "2") };
        let store = ObservationStore::new();
        store.record(sample_key("/a"), sample_exchange(b"a"));
        store.record(sample_key("/b"), sample_exchange(b"b"));
        // Retouche /a : elle devient la plus recente, /b devient la plus
        // ancienne et doit etre evincee au lieu de /a.
        store.record(sample_key("/a"), sample_exchange(b"a2"));
        store.record(sample_key("/c"), sample_exchange(b"c"));
        assert!(!store.observations(&sample_key("/a")).is_empty());
        assert!(store.observations(&sample_key("/b")).is_empty());
        unsafe { std::env::remove_var("TRAFFIC_OBSERVATION_MAX_KEYS") };
    }

    #[test]
    fn max_body_size_default_is_16kb() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("TRAFFIC_OBSERVATION_MAX_BODY_SIZE") };
        assert_eq!(max_body_size(), 16 * 1024);
    }

    #[test]
    fn exchange_truncates_bodies_beyond_max_size() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("TRAFFIC_OBSERVATION_MAX_BODY_SIZE", "3") };
        let exchange = sample_exchange(b"0123456789");
        assert!(exchange.request_body_truncated);
        assert_eq!(exchange.request_body, "012");
        assert!(exchange.response_body_truncated);
        assert_eq!(exchange.response_body, "012");
        unsafe { std::env::remove_var("TRAFFIC_OBSERVATION_MAX_BODY_SIZE") };
    }

    #[test]
    fn max_buffer_size_default_is_10mb() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("TRAFFIC_OBSERVATION_MAX_BUFFER_SIZE") };
        assert_eq!(max_buffer_size(), 10 * 1024 * 1024);
    }
}
