// No unsafe Rust in the shipped binary. Tests use it only to set environment variables (unsafe since edition
// 2024), which is why the attribute is limited to non-test builds.
#![cfg_attr(not(test), forbid(unsafe_code))]

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
pub mod engine;
#[cfg(feature = "messaging-kafka")]
pub mod messaging;
pub mod models;
pub mod server;
pub mod store;
#[cfg(feature = "tcp-mock")]
pub mod tcp;

use crate::auth::AuthConfig;
use crate::auth::keycloak::KeycloakClient;
use crate::engine::ProxyClient;
use crate::engine::script::ScriptEngine;
use crate::server::browser_guard::BrowserGuard;
use crate::server::ping::PingCache;
use crate::server::request_log::RequestLog;
use crate::server::{AppState, build_router_with};
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

    let bind_ip = match bind_address() {
        Ok(ip) => ip,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

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
            crate::messaging::consumer::spawn(
                kafka_config.clone(),
                store.clone(),
                message_log.clone(),
            )
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
    // Reconfigurable a chaud ensuite via l'API REST (server/api.rs,
    // TcpRuntime::replace) sans redemarrer le processus. Taches detachees
    // (tokio::spawn, geree par TcpRuntime) : a l'arret gracieux, seul le
    // serveur HTTP est draine proprement (with_graceful_shutdown ci-dessous)
    // puis la config flush(ee) ; les connexions TCP brutes en cours sont
    // coupees net avec le runtime, comme le serait un SIGKILL. Raisonnable
    // pour cette premiere tranche : un protocole binaire n'a pas de
    // mecanisme generique de "fin de session propre" a declencher cote
    // client de toute facon.
    #[cfg(feature = "tcp-mock")]
    let tcp_runtime = crate::tcp::TcpRuntime::load_and_spawn(&data_dir).await;

    let state = AppState {
        store,
        proxy: ProxyClient::new(),
        seq_counters: Arc::new(RwLock::new(HashMap::new())),
        request_log: RequestLog::new(),
        auth_config,
        keycloak,
        script_engine: ScriptEngine::new(),
        ping_cache: PingCache::new(),
        observation: crate::server::observation::ObservationState::new(),
        #[cfg(feature = "messaging-kafka")]
        messaging,
        #[cfg(feature = "tcp-mock")]
        tcp_runtime,
    };

    let store_for_shutdown = state.store.clone();
    let guard = BrowserGuard::from_env().with_loopback_hosts_only(bind_ip.is_loopback());
    let app = build_router_with(state, &static_dir, guard);
    let addr = std::net::SocketAddr::new(bind_ip, port);

    if bind_ip.is_loopback() {
        tracing::info!(
            addr = %addr,
            "lightMock listening on this machine only (set BIND_ADDRESS=0.0.0.0 to accept remote connections)"
        );
    } else {
        tracing::info!(addr = %addr, "lightMock listening");
    }

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

// Loopback by default: without authentication (the default), a server reachable from the network lets anyone on it
// rewrite the mocks and use the proxy rules. Containers set BIND_ADDRESS=0.0.0.0, the network being theirs.
fn bind_address() -> Result<std::net::IpAddr, String> {
    let raw = std::env::var("BIND_ADDRESS").unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(std::net::Ipv4Addr::LOCALHOST.into());
    }
    raw.parse().map_err(|_| {
        format!(
            "BIND_ADDRESS must be an IP address such as 127.0.0.1, 0.0.0.0 or ::, got \"{raw}\""
        )
    })
}

// Kubernetes, Docker and systemd stop a process with SIGTERM, not SIGINT: listening to Ctrl+C alone let every
// pod stop kill the process before the write-behind drain above, losing the last queued configuration writes.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "cannot listen for Ctrl+C");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "cannot listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("shutdown signal received");
}
