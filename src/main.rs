// Modules du projet — chaque dossier src/<module>/ contient un mod.rs
// Pour modifier un comportement, trouver le module correspondant :
//   auth/      → authentification Keycloak, permissions groupes
//   models/    → structures de donnees (Service, Rule, Group, etc.)
//   engine/    → moteur de matching, proxy HTTP, template, scripts rhai
//   store/     → persistance YAML sur disque
//   server/    → API REST (routes /api/*), middleware d'interception HTTP
//   messaging/ → cadrage MOM/Kafka (etude), feature "messaging-kafka" NON
//                active par defaut — ce module ne compile meme pas sinon
pub mod auth;
pub mod models;
pub mod engine;
pub mod store;
pub mod server;
#[cfg(feature = "messaging-kafka")]
pub mod messaging;
#[cfg(feature = "tcp-proxy")]
pub mod tcp;

use crate::auth::AuthConfig;
use crate::auth::keycloak::KeycloakClient;
use crate::engine::ProxyClient;
use crate::engine::script::ScriptEngine;
use crate::server::ping::PingCache;
use crate::server::request_log::RequestLog;
use crate::server::{AppState, build_router};
use crate::store::MockStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

// Point d'entree — lit la config depuis les variables d'environnement,
// initialise le store YAML, le proxy HTTP, l'auth Keycloak (si active),
// le moteur de scripts rhai, puis demarre le serveur Axum.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "light_mock=info".parse().unwrap()),
        )
        .init();

    let data_dir = MockStore::data_path();
    let store = MockStore::load_or_init(&data_dir)
        .await
        .expect("failed to load config");

    let static_dir = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./frontend/dist"));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7342);

    let auth_config = AuthConfig::from_env();
    let keycloak = if auth_config.enabled {
        tracing::info!(
            keycloak_url = %auth_config.keycloak_url,
            realm = %auth_config.realm,
            "auth enabled, connecting to Keycloak"
        );
        Some(KeycloakClient::new(auth_config.clone()))
    } else {
        tracing::info!("auth disabled");
        None
    };

    #[cfg(feature = "messaging-kafka")]
    let messaging = {
        let kafka_config = crate::messaging::KafkaConfig::from_env();
        let message_log = crate::messaging::message_log::MessageLog::new();
        let publisher = if kafka_config.enabled {
            tracing::info!(
                topic = %kafka_config.listen_topic,
                brokers = ?kafka_config.brokers,
                "messaging: Kafka enabled, starting consumer"
            );
            crate::messaging::consumer::spawn(kafka_config.clone(), store.clone(), message_log.clone())
        } else {
            tracing::info!("messaging: Kafka disabled (KAFKA_ENABLED=false)");
            crate::messaging::consumer::Publisher::None
        };
        crate::messaging::MessagingState {
            message_log,
            reply_topic: kafka_config.reply_topic,
            publisher,
        }
    };

    // Ecoutes TCP brutes (protocoles binaires non-HTTP), en parallele du
    // serveur HTTP : jamais sur le meme port, un service TCP a le sien.
    // Taches detachees (tokio::spawn) : a l'arret gracieux, seul le serveur
    // HTTP est draine proprement (with_graceful_shutdown ci-dessous) puis la
    // config flush(ee) ; les connexions TCP brutes en cours sont coupees net
    // avec le runtime, comme le serait un SIGKILL. Raisonnable pour cette
    // premiere tranche : un protocole binaire n'a pas de mecanisme generique
    // de "fin de session propre" a declencher cote client de toute facon.
    // Demarre AVANT la construction d'AppState : le statut par service
    // (`tcp_status`, expose via GET /tcp/status) est fige a cet instant et
    // porte dans l'etat partage, comme le reste.
    #[cfg(feature = "tcp-proxy")]
    let tcp_status = {
        let tcp_config = crate::tcp::config::TcpConfig::load(&data_dir);
        if !tcp_config.services.is_empty() {
            tracing::info!(
                count = tcp_config.services.len(),
                "tcp-proxy: starting configured services"
            );
        }
        let (_handles, statuses) = crate::tcp::spawn_tcp_services(&tcp_config).await;
        Arc::new(statuses)
    };

    let state = AppState {
        store,
        proxy: ProxyClient::new(),
        seq_counters: Arc::new(RwLock::new(HashMap::new())),
        request_log: RequestLog::new(),
        auth_config,
        keycloak,
        script_engine: ScriptEngine::new(),
        ping_cache: PingCache::new(),
        #[cfg(feature = "messaging-kafka")]
        messaging,
        #[cfg(feature = "tcp-proxy")]
        tcp_status,
    };

    let store_for_shutdown = state.store.clone();
    let app = build_router(state, &static_dir);
    let addr = format!("0.0.0.0:{port}");

    tracing::info!(addr = %addr, "lightMock listening");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    // Arret gracieux (SIGTERM K8s) : draine la file d'ecriture write-behind
    // avant de quitter, pour reduire la fenetre de risque de perte des
    // dernieres mutations en cas d'arret normal du pod. Ne protege pas
    // contre un SIGKILL/crash brutal.
    tracing::info!("draining pending config writes before exit");
    store_for_shutdown.flush().await;
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("shutdown signal received");
}
