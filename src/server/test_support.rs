//! Builders for the tests that run the real router on a real socket, so that they exercise the middleware stack
//! exactly as it runs in production.
use crate::auth::AuthConfig;
use crate::models::{MockConfig, Service};
use crate::server::AppState;
use axum::Router;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub(crate) fn temp_data_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lightmock-{prefix}-{}", fastrand::u64(..)));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn auth_disabled() -> AuthConfig {
    AuthConfig {
        enabled: false,
        keycloak_url: String::new(),
        realm: String::new(),
        client_id: String::new(),
        super_admins: vec![],
        show_reset_button: false,
    }
}

/// An ungrouped, mocked service whose single `GET` rule answers `body` on any path.
pub(crate) fn mock_service(name: &str, body: &str) -> Service {
    serde_json::from_value(serde_json::json!({
        "name": name, "listen_path": "", "real_target_url": "", "is_mocked": true,
        "rewrite_directory_urls": false, "group_name": null, "wsdl_mode": "auto",
        "rules": [{
            "name": "any", "method": "GET", "sub_path": null, "action": "mock",
            "pre_script": null, "script": null, "post_script": null,
            "conditions": {"all_of": [], "any_of": []},
            "response": {"status": 200, "headers": [], "body": [{"type": "Literal", "value": body}]}
        }]
    }))
    .unwrap()
}

pub(crate) async fn test_state(
    data_dir: &std::path::Path,
    config: MockConfig,
    auth_config: AuthConfig,
) -> AppState {
    let store = crate::store::MockStore::new(data_dir.join("mock-config.yaml"));
    store.replace(config).await.unwrap();
    store.flush().await;
    AppState {
        store,
        proxy: crate::engine::ProxyClient::new(),
        seq_counters: Arc::new(RwLock::new(HashMap::new())),
        request_log: crate::server::request_log::RequestLog::new(),
        auth_config,
        keycloak: None,
        script_engine: crate::engine::script::ScriptEngine::new(),
        ping_cache: crate::server::ping::PingCache::new(),
        observation: crate::server::observation::ObservationState::new(),
        #[cfg(feature = "messaging-kafka")]
        messaging: crate::messaging::MessagingState {
            message_log: crate::messaging::message_log::MessageLog::new(),
            reply_topic: None,
            publisher: crate::messaging::consumer::Publisher::None,
        },
        #[cfg(feature = "tcp-mock")]
        tcp_runtime: crate::tcp::TcpRuntime::load_and_spawn(data_dir).await,
    }
}

/// Serves `app` on a free local port and returns its root URL (`http://127.0.0.1:<port>`).
pub(crate) async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://127.0.0.1:{port}")
}
