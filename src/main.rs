// No unsafe Rust in the shipped binary. Tests use it only to set environment variables (unsafe since edition
// 2024), which is why the attribute is limited to non-test builds.
#![cfg_attr(not(test), forbid(unsafe_code))]

//   auth/      Keycloak token validation, group permissions
//   models/    configuration schema (Service, Rule, Group...)
//   engine/    matching, HTTP proxy, templates, Rhai scripts
//   store/     YAML persistence, backups
//   server/    management API (/api/*), service interception, browser guard
//   i18n       server messages in the language of the request
//   messaging/ Kafka, compiled only with the "messaging-kafka" feature
//   tcp/       raw TCP mocks, compiled only with the "tcp-mock" feature
pub mod auth;
pub mod engine;
pub mod i18n;
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

// Reads the configuration from the environment, loads the stored mocks, builds the proxy, the Keycloak client (when
// authentication is on) and the script engine, then serves HTTP until SIGTERM or Ctrl+C.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "light_mock=info".parse().unwrap()),
        )
        .init();

    let bind_ip = bind_address().unwrap_or_else(|message| exit_with(&message));

    let auth_config = AuthConfig::from_env().unwrap_or_else(|message| exit_with(&message));

    let data_dir = MockStore::data_path();
    let store = MockStore::load_or_init(&data_dir)
        .await
        .unwrap_or_else(|e| {
            exit_with(&format!(
                "cannot load the configuration from {} (DATA_PATH): {e}",
                data_dir.display()
            ))
        });

    let static_dir = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./frontend/dist"));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7342);

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

    // Raw TCP mocks listen on their own ports, next to the HTTP server, and are reconfigured live through the API
    // (TcpRuntime::replace). Their tasks are detached: on shutdown only HTTP is drained, and open TCP connections are
    // cut, since a binary protocol has no generic way to end a session cleanly from the server side anyway.
    #[cfg(feature = "tcp-mock")]
    let tcp_runtime = crate::tcp::TcpRuntime::load_and_spawn(&data_dir, bind_ip).await;

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
        .unwrap_or_else(|e| {
            exit_with(&format!(
                "cannot listen on {addr}: {e} (another process may use the port; see PORT and BIND_ADDRESS)"
            ))
        });

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!(error = %e, "server stopped on an error");
    }

    // Changes are written to disk in the background: drain that queue before exiting, so a normal stop keeps the last
    // changes. A SIGKILL or a crash can still lose the writes still queued.
    tracing::info!("draining pending config writes before exit");
    store_for_shutdown.flush().await;
}

/// Stops the start with a message meant for whoever configures the process, instead of a panic and its trace.
fn exit_with(message: &str) -> ! {
    eprintln!("lightMock cannot start: {message}");
    std::process::exit(2)
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
