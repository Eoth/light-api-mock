// Serveur HTTP Axum — contient le routeur, les handlers API, et le middleware d'interception.
//   api.rs       → handlers REST (/api/services, /api/groups, /api/auth, etc.)
//   intercept.rs → middleware qui intercepte les requetes et les route vers mock ou proxy
//   validation.rs → validation des noms de services, methodes HTTP, etc.
//   request_log.rs → journal en memoire des 200 dernieres requetes interceptees
mod api;
pub(crate) mod codegen;
mod intercept;
pub mod ping;
pub mod request_log;
pub mod validation;

use crate::auth::AuthConfig;
use crate::auth::keycloak::KeycloakClient;
use crate::engine::ProxyClient;
use crate::engine::script::ScriptEngine;
use crate::store::MockStore;
use axum::Router;
use ping::PingCache;
use request_log::RequestLog;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub store: MockStore,
    pub proxy: ProxyClient,
    pub seq_counters: Arc<RwLock<HashMap<String, Arc<AtomicU64>>>>,
    pub request_log: RequestLog,
    pub auth_config: AuthConfig,
    pub keycloak: Option<KeycloakClient>,
    pub script_engine: ScriptEngine,
    pub ping_cache: PingCache,
    #[cfg(feature = "messaging-kafka")]
    pub messaging: crate::messaging::MessagingState,
}

impl AppState {
    pub fn next_seq(&self, service_name: &str) -> u64 {
        {
            let counters = self.seq_counters.read().unwrap();
            if let Some(counter) = counters.get(service_name) {
                return counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        let mut counters = self.seq_counters.write().unwrap();
        let counter = counters
            .entry(service_name.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

// Reponse de `GET /runtime-config.json` — voir le commentaire au-dessus de
// `runtime_config_handler` pour le detail complet de ce mecanisme.
#[derive(serde::Serialize)]
struct RuntimeConfig {
    api_base_url: String,
}

// Permet de configurer l'URL de base que le frontend utilise pour appeler
// l'API (`/api/*`) INDEPENDAMMENT du Host sur lequel la SPA elle-meme est
// chargee. Sans configuration, le frontend continue de deriver l'URL de
// l'API de son propre Host (comportement historique, URL relative) : ca
// fonctionne quand front et back sont co-localises (deploiement par defaut),
// mais casse des que l'infrastructure route `/api` vers une origine
// distincte de celle qui sert les assets statiques (ex. K8s/Gloo Edge avec
// un VirtualService pour le front et un RouteTable/Upstream separe pour le
// back).
//
// Servi EN DEHORS de `/api` (route enregistree directement sur le Router
// racine, pas nestee sous `api::routes()`) et ajoute a `is_internal_route`/
// `is_static_asset_route` (src/server/validation.rs) : ce fichier doit
// rester joignable meme quand `/api` est route vers une origine differente
// par l'infrastructure — il doit arriver au frontend par le MEME chemin que
// index.html/le bundle JS (c'est ce qui lui permet, une fois charge,
// d'apprendre ou se trouve l'API). Pour la meme raison il est exempte
// d'authentification : le frontend doit pouvoir le lire avant meme de
// savoir s'il est connecte.
//
// Configuration au niveau du CONTENEUR (variable d'environnement
// `API_BASE_URL`, lue directement ici a chaque requete), pas au moment du
// BUILD : la meme image Docker, buildee une seule fois, peut ainsi etre
// configuree differemment par environnement de deploiement sans rebuild.
// Lu directement via `std::env::var` plutot que mis en cache dans
// `AppState` : evite d'ajouter un champ a AppState et a ses ~11 sites de
// construction dans les tests, pour un parametre qui ne varie jamais en
// cours d'execution d'un pod — meme discipline que BACKUP_MAX_COUNT/
// MESSAGE_LOG_TTL_MS.
async fn runtime_config_handler() -> axum::Json<RuntimeConfig> {
    let api_base_url = std::env::var("API_BASE_URL")
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string();
    axum::Json(RuntimeConfig { api_base_url })
}

pub fn build_router(state: AppState, static_dir: &Path) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = api::routes();

    let auth_config = state.auth_config.clone();
    let keycloak = state.keycloak.clone();

    Router::new()
        .route("/runtime-config.json", axum::routing::get(runtime_config_handler))
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            intercept::intercept_layer,
        ))
        .layer(axum::middleware::from_fn(move |req, next| {
            crate::auth::middleware::auth_middleware(
                auth_config.clone(),
                keycloak.clone(),
                req,
                next,
            )
        }))
        .with_state(state)
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;

    // API_BASE_URL est une variable d'environnement process-wide : les tests
    // qui la mutent doivent tenir ce mutex pour tout leur corps sans quoi deux
    // tests concurrents (cargo test lance les fns de test en parallele) se
    // marchent dessus de facon intermittente.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn runtime_config_defaults_to_empty_when_env_unset() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("API_BASE_URL") };
        let axum::Json(config) = runtime_config_handler().await;
        assert_eq!(config.api_base_url, "");
    }

    #[tokio::test]
    async fn runtime_config_returns_configured_value() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("API_BASE_URL", "https://api.example.com") };
        let axum::Json(config) = runtime_config_handler().await;
        unsafe { std::env::remove_var("API_BASE_URL") };
        assert_eq!(config.api_base_url, "https://api.example.com");
    }

    #[tokio::test]
    async fn runtime_config_trims_trailing_slash_and_whitespace() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("API_BASE_URL", "  https://api.example.com/  ") };
        let axum::Json(config) = runtime_config_handler().await;
        unsafe { std::env::remove_var("API_BASE_URL") };
        assert_eq!(config.api_base_url, "https://api.example.com");
    }

    // runtime_config_handler n'a volontairement aucune dependance a AppState
    // (lit l'env directement) : un nouveau champ obligatoire sur AppState
    // ajouterait un site de construction de plus a maintenir dans les tests.
    async fn spawn_test_app(auth_config: crate::auth::AuthConfig) -> String {
        let data_dir = std::env::temp_dir().join(format!(
            "lightmock-servermod-test-{}",
            fastrand::u64(..)
        ));
        std::fs::create_dir_all(&data_dir).unwrap();
        let store = crate::store::MockStore::new(data_dir.join("mock-config.yaml"));
        store
            .replace(crate::models::MockConfig { services: vec![], groups: vec![] })
            .await
            .unwrap();
        store.flush().await;

        #[cfg(feature = "messaging-kafka")]
        let messaging = crate::messaging::MessagingState {
            message_log: crate::messaging::message_log::MessageLog::new(),
            reply_topic: None,
            publisher: crate::messaging::consumer::Publisher::None,
        };
        let state = AppState {
            store,
            proxy: crate::engine::ProxyClient::new(),
            seq_counters: Arc::new(RwLock::new(HashMap::new())),
            request_log: RequestLog::new(),
            auth_config,
            keycloak: None,
            script_engine: crate::engine::script::ScriptEngine::new(),
            ping_cache: PingCache::new(),
            #[cfg(feature = "messaging-kafka")]
            messaging,
        };
        let app = build_router(state, &data_dir);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://127.0.0.1:{port}")
    }

    #[tokio::test]
    async fn runtime_config_route_accessible_without_token_when_auth_enabled() {
        // Meme si AUTH_ENABLED=true et qu'aucun KeycloakClient n'est
        // configure (auth_middleware ferait echouer TOUTE autre route
        // protegee en 500, cf point "fail closed" de middleware.rs), cette
        // route doit rester accessible sans token : elle doit pouvoir etre
        // lue avant meme de savoir si l'utilisateur est authentifie.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("API_BASE_URL", "https://api.example.com") };
        let auth_config = crate::auth::AuthConfig {
            enabled: true,
            keycloak_url: "http://127.0.0.1:1".into(),
            realm: "test-realm".into(),
            client_id: "lightmock".into(),
            super_admins: vec![],
            show_reset_button: false,
        };
        let base = spawn_test_app(auth_config).await;
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/runtime-config.json"))
            .send()
            .await
            .unwrap();
        unsafe { std::env::remove_var("API_BASE_URL") };
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["api_base_url"], "https://api.example.com");
    }

    #[tokio::test]
    async fn runtime_config_route_not_intercepted_as_a_mock_service() {
        // Preuve bout-en-bout que la route traverse bien intercept_layer
        // (is_internal_route) sans jamais etre evaluee contre les services
        // configures, meme quand des services existent.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("API_BASE_URL") };
        let auth_config = crate::auth::AuthConfig {
            enabled: false,
            keycloak_url: String::new(),
            realm: String::new(),
            client_id: String::new(),
            super_admins: vec![],
            show_reset_button: false,
        };
        let base = spawn_test_app(auth_config).await;
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/runtime-config.json"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["api_base_url"], "");
    }
}
