mod api;
mod intercept;
pub mod request_log;
pub mod validation;

use crate::engine::ProxyClient;
use crate::store::MockStore;
use axum::Router;
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

pub fn build_router(state: AppState, static_dir: &Path) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = api::routes();

    Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new(static_dir).append_index_html_on_directories(true))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            intercept::intercept_layer,
        ))
        .with_state(state)
        .layer(cors)
}
