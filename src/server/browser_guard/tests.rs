use super::*;
use crate::models::MockConfig;
use crate::server::test_support::{auth_disabled, mock_service, serve, temp_data_dir, test_state};

const UI: &str = "https://ui.example.com";
const EVIL: &str = "https://evil.example";

fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(*name, HeaderValue::from_str(value).unwrap());
    }
    map
}

fn refuses(guard: &BrowserGuard, method: &Method, path: &str, headers: &HeaderMap) -> bool {
    guard.refusal(method, path, headers).is_some()
}

#[test]
fn parses_a_comma_separated_list_and_ignores_blanks_and_trailing_slashes() {
    let guard = BrowserGuard::new(" https://ui.example.com/ , ,http://localhost:5173");
    assert!(guard.allows(&HeaderValue::from_static("https://ui.example.com")));
    assert!(guard.allows(&HeaderValue::from_static("http://localhost:5173")));
    assert!(!guard.allows(&HeaderValue::from_static("http://localhost:5174")));
    assert!(BrowserGuard::new("").allowed_origins.is_empty());
}

#[test]
fn refuses_cross_site_writes_to_the_api_only() {
    let guard = BrowserGuard::new(UI);
    let cross = headers(&[("sec-fetch-site", "cross-site"), ("origin", EVIL)]);
    assert!(refuses(&guard, &Method::POST, "/api/services", &cross));
    assert!(refuses(
        &guard,
        &Method::DELETE,
        "/api/config/reset",
        &cross
    ));
    assert!(refuses(
        &guard,
        &Method::PUT,
        "/api/config",
        &headers(&[("sec-fetch-site", "same-site")])
    ));
    // Reads, service traffic, same-origin calls and non-browser clients go through.
    assert!(!refuses(&guard, &Method::GET, "/api/config", &cross));
    assert!(!refuses(&guard, &Method::POST, "/orders/42", &cross));
    assert!(!refuses(
        &guard,
        &Method::POST,
        "/api/services",
        &headers(&[("sec-fetch-site", "same-origin")])
    ));
    assert!(!refuses(
        &guard,
        &Method::POST,
        "/api/services",
        &HeaderMap::new()
    ));
    // A listed origin may write from another site.
    let listed = headers(&[("sec-fetch-site", "cross-site"), ("origin", UI)]);
    assert!(!refuses(&guard, &Method::POST, "/api/services", &listed));
}

async fn spawn_app() -> String {
    let data_dir = temp_data_dir("browser-guard");
    let config = MockConfig {
        services: vec![mock_service("orders", "mocked")],
        groups: vec![],
    };
    let state = test_state(&data_dir, config, auth_disabled()).await;
    serve(crate::server::build_router_with(
        state,
        crate::server::ui_files::UiSource::Directory(data_dir.clone()),
        BrowserGuard::new(UI),
    ))
    .await
}

async fn preflight(client: &reqwest::Client, url: String, origin: &str) -> Option<String> {
    let resp = client
        .request(Method::OPTIONS, url)
        .header("origin", origin)
        .header("access-control-request-method", "PUT")
        .send()
        .await
        .unwrap();
    resp.headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap().to_string())
}

#[tokio::test]
async fn api_cors_is_limited_to_listed_origins_while_mocks_stay_open() {
    let root = spawn_app().await;
    let client = reqwest::Client::new();
    assert_eq!(
        preflight(&client, format!("{root}/api/config"), EVIL).await,
        None
    );
    assert_eq!(
        preflight(&client, format!("{root}/api/config"), UI)
            .await
            .as_deref(),
        Some(UI)
    );
    assert_eq!(
        preflight(&client, format!("{root}/orders/42"), EVIL)
            .await
            .as_deref(),
        Some(EVIL)
    );
}

#[tokio::test]
async fn a_cross_site_page_cannot_change_the_configuration() {
    let root = spawn_app().await;
    let client = reqwest::Client::new();
    let reset = |language: &'static str| {
        client
            .delete(format!("{root}/api/config/reset"))
            .header("origin", EVIL)
            .header("sec-fetch-site", "cross-site")
            .header("accept-language", language)
            .send()
    };
    for (language, reason) in [
        ("en-US", "Cross-site request refused"),
        ("fr-FR", "Requête inter-sites refusée"),
    ] {
        let resp = reset(language).await.unwrap();
        assert_eq!(resp.status().as_u16(), 403);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(
            body["error"].as_str().unwrap().starts_with(reason),
            "{body}"
        );
    }
    let services = client
        .get(format!("{root}/api/services"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(
        services.as_array().map(Vec::len),
        Some(1),
        "nothing was reset"
    );

    let mocked = client
        .get(format!("{root}/orders/42"))
        .header("origin", EVIL)
        .header("sec-fetch-site", "cross-site")
        .send()
        .await
        .unwrap();
    assert_eq!(mocked.text().await.unwrap(), "mocked");
}

#[test]
fn recognizes_loopback_host_headers() {
    for host in [
        "localhost",
        "localhost:7342",
        "LOCALHOST:7342",
        "app.localhost:5173",
        "127.0.0.1:7342",
        "127.1.2.3",
        "[::1]:7342",
        "[::1]",
    ] {
        assert!(is_loopback_host(host), "{host}");
    }
    for host in [
        "evil.example:7342",
        "localhost.evil.example",
        "10.0.0.5:7342",
        "[::2]:7342",
        "",
    ] {
        assert!(!is_loopback_host(host), "{host}");
    }
}

#[test]
fn a_loopback_only_server_answers_the_api_through_loopback_names_only() {
    let guard = BrowserGuard::new("").with_loopback_hosts_only(true);
    let rebound = headers(&[("host", "evil.example:7342")]);
    assert!(refuses(&guard, &Method::GET, "/api/config", &rebound));
    assert!(!refuses(&guard, &Method::GET, "/orders/42", &rebound));
    assert!(!refuses(
        &guard,
        &Method::GET,
        "/api/config",
        &headers(&[("host", "localhost:7342")])
    ));
    // Listening on every interface: the operator chose the names, nothing to check.
    assert!(!refuses(
        &BrowserGuard::new(""),
        &Method::GET,
        "/api/config",
        &rebound
    ));
}

#[tokio::test]
async fn mimicway_pages_carry_security_headers_but_mock_responses_stay_untouched() {
    let root = spawn_app().await;
    let client = reqwest::Client::new();
    for path in ["/api/health", "/runtime-config.json", "/index.html"] {
        let resp = client.get(format!("{root}{path}")).send().await.unwrap();
        let headers = resp.headers();
        let csp = headers["content-security-policy"].to_str().unwrap();
        assert!(csp.contains("frame-ancestors 'none'"), "{path}: {csp}");
        assert!(csp.contains("script-src 'self'"), "{path}: {csp}");
        assert_eq!(headers["x-content-type-options"], "nosniff", "{path}");
        assert_eq!(headers["x-frame-options"], "DENY", "{path}");
    }
    let mocked = client
        .get(format!("{root}/orders/42"))
        .send()
        .await
        .unwrap();
    for name in [
        "content-security-policy",
        "x-frame-options",
        "x-content-type-options",
    ] {
        assert!(
            mocked.headers().get(name).is_none(),
            "{name} added to a mock response"
        );
    }
}

#[test]
fn the_ui_may_call_the_configured_api_origin() {
    let guard = BrowserGuard {
        api_origin: Some("https://api.example.com".into()),
        ..BrowserGuard::new("")
    };
    assert!(
        guard
            .content_security_policy()
            .contains("connect-src 'self' https://api.example.com;")
    );
    assert!(
        BrowserGuard::new("")
            .content_security_policy()
            .contains("connect-src 'self';")
    );
}
