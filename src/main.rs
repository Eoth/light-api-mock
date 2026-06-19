pub mod models;
pub mod engine;
pub mod store;
pub mod server;

use crate::engine::ProxyClient;
use crate::server::{AppState, build_router};
use crate::store::MockStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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
        .unwrap_or(3000);

    let state = AppState {
        store,
        proxy: ProxyClient::new(),
        seq_counters: Arc::new(RwLock::new(HashMap::new())),
    };

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
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("shutdown signal received");
}
