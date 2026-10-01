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
    assert!(guard.refuses(&Method::POST, "/api/services", &cross));
    assert!(guard.refuses(&Method::DELETE, "/api/config/reset", &cross));
    assert!(guard.refuses(
        &Method::PUT,
        "/api/config",
        &headers(&[("sec-fetch-site", "same-site")])
    ));
    // Reads, service traffic, same-origin calls and non-browser clients go through.
    assert!(!guard.refuses(&Method::GET, "/api/config", &cross));
    assert!(!guard.refuses(&Method::POST, "/orders/42", &cross));
    assert!(!guard.refuses(
        &Method::POST,
        "/api/services",
        &headers(&[("sec-fetch-site", "same-origin")])
    ));
    assert!(!guard.refuses(&Method::POST, "/api/services", &HeaderMap::new()));
    // A listed origin may write from another site.
    let listed = headers(&[("sec-fetch-site", "cross-site"), ("origin", UI)]);
    assert!(!guard.refuses(&Method::POST, "/api/services", &listed));
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
        &data_dir,
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
    let resp = client
        .delete(format!("{root}/api/config/reset"))
        .header("origin", EVIL)
        .header("sec-fetch-site", "cross-site")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 403);
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
