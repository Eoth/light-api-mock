//! Builders for the tests that run the real router on a real socket, so that they exercise the middleware stack
//! exactly as it runs in production.
use crate::auth::AuthConfig;
use crate::models::{MockConfig, Service};
use crate::server::AppState;
use axum::Router;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// A fixture with a service in a group it does not define does not run like production (no group code, so
/// another URL): it once hid the fact that traffic observation never worked for grouped services.
pub(crate) fn assert_consistent(config: &MockConfig) {
    let dangling = config.unknown_group_references();
    assert!(
        dangling.is_empty(),
        "test fixture refers to undefined groups: {dangling:?}"
    );
}

/// Directory of the current test run: `<temp>/mimicway-tests/<pid>-<start time>`. A test cannot know when the
/// servers it started stop writing, so its directory is not removed by the test itself; instead the first test of
/// each run removes the runs older than an hour. Tests used to leave one directory per test in the temporary
/// folder, thousands after a few days.
static RUN_DIR: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
    let root = std::env::temp_dir().join("mimicway-tests");
    if let Ok(runs) = std::fs::read_dir(&root) {
        let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for run in runs.flatten() {
            let stale = run
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|modified| modified < hour_ago);
            if stale {
                let _ = std::fs::remove_dir_all(run.path());
            }
        }
    }
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    root.join(format!("{}-{started}", std::process::id()))
});

/// A fresh, empty directory for one test, under the directory of the current run.
pub(crate) fn temp_data_dir(prefix: &str) -> PathBuf {
    let dir = RUN_DIR.join(format!("{prefix}-{}", fastrand::u64(..)));
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
        issuer: String::new(),
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
    assert_consistent(&config);
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
        tcp_runtime: crate::tcp::TcpRuntime::load_and_spawn(data_dir, crate::tcp::LOOPBACK).await,
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
