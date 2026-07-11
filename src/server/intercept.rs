use crate::engine::matcher::match_path;
use crate::engine::script::{ScriptContext, ScriptEngine, ScriptResult};
use crate::engine::{apply_chaos_and_render, MatchEngine, RequestData, TemplateContext};
use crate::models::{RuleAction, Service, WsdlMode};
use crate::server::AppState;
use crate::server::request_log::CapturedRequest;
use crate::server::validation::is_internal_route;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

/// Execute un des 3 blocs de script d'une regle (pre_script/script/post_script),
/// independamment des autres (meme ScriptContext, pas de chainage — voir
/// commentaire sur Rule dans models/mod.rs). `None` si le slot n'a pas de
/// script. En cas d'erreur d'execution, log + repli sur un resultat vide
/// (soft-fail : la requete continue, jamais bloquee par un script casse).
fn run_rule_script(
    engine: &ScriptEngine,
    rule_name: &str,
    slot: &str,
    script: &Option<String>,
    ctx: &ScriptContext,
) -> Option<ScriptResult> {
    let script = script.as_ref()?;
    match engine.execute(script, ctx) {
        Ok(result) => Some(result),
        Err(e) => {
            tracing::warn!(rule = rule_name, slot, error = %e, "script execution failed");
            Some(ScriptResult::default())
        }
    }
}

// Middleware Axum execute sur CHAQUE requete HTTP entrante.
// Pipeline : route interne? → skip | chercher service par path → mock ou proxy
// Les regles (Rule) sont evaluees dans l'ordre (first-match) si le service est en mode mock.
pub async fn intercept_layer(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    if is_internal_route(&path) {
        tracing::trace!(path = %path, "internal route protected, skipping intercept");
        return next.run(req).await;
    }

    let config = state.store.snapshot().await;

    let matched = config.services.iter().find_map(|s| {
        let group_code = s.group_name.as_ref().and_then(|gn| {
            config.groups.iter().find(|g| &g.name == gn)
                .map(|g| g.code.clone())
                .filter(|c| !c.trim().is_empty())
        });
        let effective = build_effective_pattern(group_code.as_deref(), &s.name, &s.listen_path);
        match_path(&effective, &path).map(|(params, remaining)| (s.clone(), params, remaining, group_code))
    });

    match matched {
        Some((service, path_params, remaining, group_code)) => {
            handle_service(&state, &service, &path, path_params, remaining, group_code, &method, req).await
        }
        None => next.run(req).await,
    }
}

fn build_effective_pattern(group_code: Option<&str>, name: &str, listen_path: &str) -> String {
    let lp = listen_path.trim().trim_start_matches('/');
    let base = match group_code {
        Some(code) => format!("/{code}/{name}"),
        None => format!("/{name}"),
    };
    if lp.is_empty() || lp == "*" {
        format!("{base}/*")
    } else if lp.ends_with('*') || lp.contains('{') {
        format!("{base}/{lp}")
    } else {
        format!("{base}/{lp}/*")
    }
}

async fn do_proxy(
    state: &AppState,
    service: &Service,
    path: &str,
    method_str: &str,
    context: &str,
    group_code: Option<&str>,
    req: Request<Body>,
    captured: Option<CapturedRequest>,
) -> Response {
    let prefix = match group_code {
        Some(code) => format!("/{}/{}", code, service.name),
        None => format!("/{}", service.name),
    };
    let proxy_path = path.strip_prefix(&prefix).unwrap_or(path);
    let target = format!(
        "{}/{}",
        service.real_target_url.trim_end_matches('/'),
        proxy_path.trim_start_matches('/')
    );
    tracing::info!(
        service_key = %service.name,
        method = %method_str,
        path = %path,
        mode = "proxy",
        context = %context,
        target = %target,
        "proxy forwarding"
    );
    match state
        .proxy
        .forward(&service.real_target_url, proxy_path, req)
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            state
                .request_log
                .log_proxy(&service.name, method_str, path, &target, status, captured);
            resp
        }
        Err(status) => {
            state.request_log.log_proxy(
                &service.name,
                method_str,
                path,
                &target,
                status.as_u16(),
                captured,
            );
            status.into_response()
        }
    }
}

async fn handle_service(
    state: &AppState,
    service: &Service,
    path: &str,
    path_params: HashMap<String, String>,
    remaining: String,
    group_code: Option<String>,
    method: &axum::http::Method,
    req: Request<Body>,
) -> Response {
    let method_str = method.to_string();
    let gc = group_code.as_deref();

    if !service.is_mocked {
        // Proxy niveau service : chemin streame sans buffering (aucun
        // RequestData construit ici), donc aucun detail capturable pour le
        // testeur de regle sur ce chemin ("Proxy streaming").
        return do_proxy(state, service, path, &method_str, "service-level", gc, req, None).await;
    }

    if is_wsdl_request(req.uri().query()) {
        match service.wsdl_mode {
            WsdlMode::Mock => {
                tracing::info!(
                    service_key = %service.name, method = %method_str, path = %path,
                    "WSDL request, mode=mock, applying rules"
                );
            }
            WsdlMode::Auto | WsdlMode::Proxy => {
                tracing::info!(
                    service_key = %service.name, method = %method_str, path = %path,
                    mode = "proxy", context = "wsdl-bypass",
                    "WSDL request, bypassing mock rules"
                );
                return do_proxy(state, service, path, &method_str, "wsdl-bypass", gc, req, None)
                    .await;
            }
        }
    }

    let uri = req.uri().clone();
    let query_params = extract_query_params(uri.query());
    let headers = extract_headers(req.headers());
    let content_type = headers.get("content-type").cloned();

    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let request_data = RequestData {
        query_params,
        headers,
        body: body_bytes.to_vec(),
        content_type,
        path_params: path_params.clone(),
        method: method_str.clone(),
        remaining_path: remaining,
    };

    // Construit une seule fois : le corps/headers/query/path params sont deja
    // entierement bufferises ci-dessus pour le matching (`request_data`), donc
    // retenir ce detail pour le testeur de regle ne cree aucune nouvelle
    // capture de trafic (cf CapturedRequest, src/server/request_log.rs).
    let captured = Some(CapturedRequest::from_request_data(&request_data));

    let matched = MatchEngine::first_match(&service.rules, &request_data);

    let Some((rule, sub_params)) = matched else {
        tracing::warn!(
            service_key = %service.name, method = %method_str, path = %path,
            mode = "no-rule",
            "no matching rule, returning 404"
        );
        state
            .request_log
            .log_no_rule(&service.name, &method_str, path, captured);
        return StatusCode::NOT_FOUND.into_response();
    };

    if rule.action == RuleAction::Proxy {
        tracing::info!(
            service_key = %service.name, method = %method_str, path = %path,
            rule = %rule.name, mode = "proxy",
            "rule matched with action=proxy"
        );
        let proxy_req = rebuild_request_for_proxy(&method_str, &uri, &request_data, &body_bytes);
        return do_proxy(
            state,
            service,
            path,
            &method_str,
            &format!("rule:{}", rule.name),
            gc,
            proxy_req,
            captured,
        )
        .await;
    }

    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let seq = state.next_seq(&service.name);

    let mut merged_params = path_params;
    merged_params.extend(sub_params);

    // pre_script/script/post_script s'executent independamment (meme
    // ScriptContext, pas de chainage entre eux — voir commentaire sur Rule
    // dans models/mod.rs). Meme comportement "soft-fail" pour les 3 : une
    // erreur de script est loggee mais ne bloque pas la requete.
    let script_ctx = ScriptContext {
        body: String::from_utf8_lossy(&request_data.body).into_owned(),
        headers: request_data.headers.clone(),
        query_params: request_data.query_params.clone(),
        path_params: merged_params.clone(),
    };
    let pre_script_result = run_rule_script(&state.script_engine, &rule.name, "pre_script", &rule.pre_script, &script_ctx);
    let script_result = run_rule_script(&state.script_engine, &rule.name, "script", &rule.script, &script_ctx);
    let post_script_result = run_rule_script(&state.script_engine, &rule.name, "post_script", &rule.post_script, &script_ctx);

    let ctx = TemplateContext {
        path_params: &merged_params,
        query_params: &request_data.query_params,
        headers: &request_data.headers,
        request_body: &request_data.body,
        seq_counter: seq,
        script_result: script_result.as_ref(),
        pre_script_result: pre_script_result.as_ref(),
        post_script_result: post_script_result.as_ref(),
    };

    match apply_chaos_and_render(&rule.response, &path_segments, &ctx).await {
        Ok((status, resp_headers, body)) => {
            tracing::info!(
                service_key = %service.name,
                method = %method_str,
                path = %path,
                mode = "mock",
                rule = %rule.name,
                status = %status.as_u16(),
                "request handled"
            );
            state.request_log.log_mock(
                &service.name,
                &method_str,
                path,
                &rule.name,
                status.as_u16(),
                captured,
            );
            let mut response = axum::http::Response::builder().status(status);
            for (name, value) in &resp_headers {
                if let Ok(hv) = HeaderValue::from_str(value) {
                    response = response.header(name.as_str(), hv);
                }
            }
            response
                .body(Body::from(body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(status) => {
            state.request_log.log_mock(
                &service.name,
                &method_str,
                path,
                &rule.name,
                status.as_u16(),
                captured,
            );
            (status, "chaos error injected").into_response()
        }
    }
}

fn rebuild_request_for_proxy(
    method: &str,
    uri: &axum::http::Uri,
    data: &RequestData,
    body_bytes: &[u8],
) -> Request<Body> {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(uri.clone());
    for (k, v) in &data.headers {
        if let Ok(hv) = HeaderValue::from_str(v) {
            builder = builder.header(k.as_str(), hv);
        }
    }
    builder
        .body(Body::from(body_bytes.to_vec()))
        .unwrap_or_else(|_| Request::new(Body::empty()))
}

fn is_wsdl_request(query: Option<&str>) -> bool {
    query
        .map(|q| {
            q.split('&').any(|part| {
                let key = part.split('=').next().unwrap_or("");
                key.eq_ignore_ascii_case("wsdl")
            })
        })
        .unwrap_or(false)
}

fn extract_query_params(query: Option<&str>) -> HashMap<String, String> {
    query
        .map(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_headers(headers: &axum::http::HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::matcher::match_path;
    use crate::models::{BodyFragment, ConditionGroup, MockConfig, MockResponse, Rule};
    use crate::store::MockStore;

    #[test]
    fn effective_pattern() {
        assert_eq!(
            build_effective_pattern(None, "insee", "/v4/sirene/{siret}"),
            "/insee/v4/sirene/{siret}"
        );
        assert_eq!(build_effective_pattern(None, "svc-a", "/*"), "/svc-a/*");
        assert_eq!(
            build_effective_pattern(None, "svc", "/api/v4"),
            "/svc/api/v4/*",
            "listen_path sans wildcard ni param doit ajouter /* implicitement"
        );
    }

    #[test]
    fn namespace_match() {
        let pattern = build_effective_pattern(None, "insee", "/v4/sirene/{siret}");
        let r = match_path(&pattern, "/insee/v4/sirene/44306184100047");
        assert!(r.is_some());
        let (params, remaining) = r.unwrap();
        assert_eq!(params.get("siret").unwrap(), "44306184100047");
        assert_eq!(remaining, "");
    }

    #[test]
    fn namespace_no_collision() {
        let p1 = build_effective_pattern(None, "svc-a", "/users/*");
        let p2 = build_effective_pattern(None, "svc-b", "/users/*");
        assert!(match_path(&p1, "/svc-a/users/42").is_some());
        assert!(match_path(&p1, "/svc-b/users/42").is_none());
        assert!(match_path(&p2, "/svc-b/users/42").is_some());
    }

    #[test]
    fn namespace_wildcard_remaining() {
        let pattern = build_effective_pattern(None, "api", "/*");
        let r = match_path(&pattern, "/api/foo/bar");
        assert!(r.is_some());
        let (_, remaining) = r.unwrap();
        assert_eq!(remaining, "/foo/bar");
    }

    #[test]
    fn proxy_path_strips_service_prefix_only() {
        let path = "/insee/v4/sirene/44306184100047";
        let service_name = "insee";
        let prefix = format!("/{}", service_name);
        let proxy_path = path.strip_prefix(&prefix).unwrap_or(path);
        assert_eq!(proxy_path, "/v4/sirene/44306184100047");
    }

    #[test]
    fn proxy_path_wildcard_service() {
        let path = "/api/users/42";
        let service_name = "api";
        let prefix = format!("/{}", service_name);
        let proxy_path = path.strip_prefix(&prefix).unwrap_or(path);
        assert_eq!(proxy_path, "/users/42");
    }

    #[test]
    fn proxy_path_root_only() {
        let path = "/svc";
        let service_name = "svc";
        let prefix = format!("/{}", service_name);
        let proxy_path = path.strip_prefix(&prefix).unwrap_or(path);
        assert_eq!(proxy_path, "");
    }

    #[test]
    fn proxy_path_preserves_deep_business_path() {
        let path = "/myservice/api/v2/resources/123/details";
        let service_name = "myservice";
        let prefix = format!("/{}", service_name);
        let proxy_path = path.strip_prefix(&prefix).unwrap_or(path);
        assert_eq!(proxy_path, "/api/v2/resources/123/details");
    }

    #[test]
    fn non_wildcard_remaining_is_empty() {
        let pattern = build_effective_pattern(None, "insee", "/v4/sirene/{siret}");
        let r = match_path(&pattern, "/insee/v4/sirene/44306184100047");
        assert!(r.is_some());
        let (_, remaining) = r.unwrap();
        assert_eq!(remaining, "");
    }

    #[test]
    fn extract_query_params_works() {
        let params = extract_query_params(Some("a=1&b=hello"));
        assert_eq!(params.get("a").unwrap(), "1");
        assert_eq!(params.get("b").unwrap(), "hello");
    }

    #[test]
    fn extract_query_params_none() {
        assert!(extract_query_params(None).is_empty());
    }

    #[test]
    fn extract_headers_works() {
        let mut map = axum::http::HeaderMap::new();
        map.insert("x-test", HeaderValue::from_static("value"));
        let result = extract_headers(&map);
        assert_eq!(result.get("x-test").unwrap(), "value");
    }

    // --- Security tests ---

    #[test]
    fn empty_pattern_never_matches() {
        assert!(match_path("", "/").is_none());
        assert!(match_path("", "/foo").is_none());
        assert!(match_path("/", "/").is_none());
        assert!(match_path("//", "/any").is_none());
    }

    #[test]
    fn empty_listen_path_cannot_hijack_root() {
        let pattern = build_effective_pattern(None, "hijacker", "");
        assert_eq!(pattern, "/hijacker/*");
        assert!(
            match_path(&pattern, "/").is_none(),
            "service with empty listen_path must not capture /"
        );
        assert!(match_path(&pattern, "/hijacker/any").is_some());
    }

    #[test]
    fn slash_listen_path_cannot_hijack_root() {
        let pattern = build_effective_pattern(None, "hijacker", "/");
        assert_eq!(pattern, "/hijacker/*");
        assert!(
            match_path(&pattern, "/").is_none(),
            "service with listen_path '/' must not capture /"
        );
    }

    #[test]
    fn service_cannot_match_internal_api_routes() {
        assert!(is_internal_route("/api/services"));
        assert!(is_internal_route("/api/config"));
        assert!(is_internal_route("/api/logs"));
        assert!(is_internal_route("/"));
        assert!(is_internal_route("/index.html"));
        assert!(is_internal_route("/assets/main.js"));
    }

    #[test]
    fn user_service_routes_not_internal() {
        assert!(!is_internal_route("/insee/v4/sirene/123"));
        assert!(!is_internal_route("/my-svc/foo/bar"));
        assert!(!is_internal_route("/users/42"));
    }

    #[test]
    fn empty_listen_path_produces_catchall() {
        let p = build_effective_pattern(None, "svc", "");
        assert_eq!(p, "/svc/*");
        assert!(match_path(&p, "/").is_none(), "must not capture root");
        assert!(match_path(&p, "/svc/foo").is_some());
        assert!(match_path(&p, "/svc/foo/bar").is_some());
        assert!(match_path(&p, "/other").is_none());
    }

    #[test]
    fn service_matches_any_method() {
        let pattern = build_effective_pattern(None, "svc", "/v1/*");
        assert!(
            match_path(&pattern, "/svc/v1/test").is_some(),
            "service matching is path-only, no method check"
        );
    }

    #[test]
    fn group_code_prefixes_url() {
        let pattern = build_effective_pattern(Some("qtr01"), "insee", "/v4/*");
        assert_eq!(pattern, "/qtr01/insee/v4/*");
        assert!(match_path(&pattern, "/qtr01/insee/v4/sirene").is_some());
        assert!(match_path(&pattern, "/insee/v4/sirene").is_none());
    }

    #[test]
    fn group_code_catchall() {
        let pattern = build_effective_pattern(Some("abc01"), "svc", "");
        assert_eq!(pattern, "/abc01/svc/*");
        assert!(match_path(&pattern, "/abc01/svc/foo").is_some());
        assert!(match_path(&pattern, "/svc/foo").is_none());
    }

    // --- WSDL bypass tests ---

    #[test]
    fn wsdl_query_detected() {
        assert!(is_wsdl_request(Some("wsdl")));
        assert!(is_wsdl_request(Some("WSDL")));
        assert!(is_wsdl_request(Some("Wsdl")));
        assert!(is_wsdl_request(Some("wsdl=")));
        assert!(is_wsdl_request(Some("foo=bar&wsdl")));
        assert!(is_wsdl_request(Some("WSDL&other=1")));
    }

    #[test]
    fn non_wsdl_query_ignored() {
        assert!(!is_wsdl_request(None));
        assert!(!is_wsdl_request(Some("")));
        assert!(!is_wsdl_request(Some("foo=bar")));
        assert!(!is_wsdl_request(Some("wsdlx=true")));
    }

    fn empty_script_ctx() -> ScriptContext {
        ScriptContext {
            body: String::new(),
            headers: HashMap::new(),
            query_params: HashMap::new(),
            path_params: HashMap::new(),
        }
    }

    #[test]
    fn run_rule_script_returns_none_when_no_script() {
        let engine = ScriptEngine::new();
        let result = run_rule_script(&engine, "my-rule", "pre_script", &None, &empty_script_ctx());
        assert!(result.is_none());
    }

    #[test]
    fn run_rule_script_executes_and_returns_result() {
        let engine = ScriptEngine::new();
        let script = Some(r#""hello""#.to_string());
        let result = run_rule_script(&engine, "my-rule", "script", &script, &empty_script_ctx());
        assert_eq!(result.unwrap().value, "hello");
    }

    #[test]
    fn run_rule_script_soft_fails_on_invalid_script() {
        let engine = ScriptEngine::new();
        let script = Some("this is not valid rhai (((".to_string());
        let result = run_rule_script(&engine, "my-rule", "post_script", &script, &empty_script_ctx());
        // Soft-fail : jamais None ni panique, un ScriptResult vide en repli.
        assert_eq!(result.unwrap().value, "");
    }

    #[test]
    fn pre_script_and_post_script_are_independent_slots() {
        let engine = ScriptEngine::new();
        let pre = Some(r#""PRE""#.to_string());
        let post = Some(r#""POST""#.to_string());
        let ctx = empty_script_ctx();

        let pre_result = run_rule_script(&engine, "r", "pre_script", &pre, &ctx);
        let post_result = run_rule_script(&engine, "r", "post_script", &post, &ctx);
        let script_result = run_rule_script(&engine, "r", "script", &None, &ctx);

        assert_eq!(pre_result.unwrap().value, "PRE");
        assert_eq!(post_result.unwrap().value, "POST");
        assert!(script_result.is_none());
    }

    // --- Test de non-regression bout-en-bout : proxy transmet query params,
    // headers custom, methode et corps intacts.
    // Capture la requete BRUTE recue par une fausse cible TCP en aval du vrai
    // serveur Axum (build_router), pour prouver que rien n'est perdu entre
    // l'entree HTTP et la sortie proxy — pas seulement au niveau de
    // ProxyClient::forward() en isolation (couvert separement dans
    // engine/proxy.rs), mais a travers tout le pipeline intercept_layer /
    // do_proxy / handle_service.

    fn temp_dir_for_intercept_test() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lightmock-intercept-test-{}", fastrand::u64(..)));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn capture_one_raw_request(
        ready_tx: tokio::sync::oneshot::Sender<u16>,
    ) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        ready_tx.send(addr.port()).unwrap();

        let (mut stream, _) = listener.accept().await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = tokio::time::timeout(
                std::time::Duration::from_millis(1000),
                stream.read(&mut chunk),
            )
            .await
            .unwrap_or(Ok(0))
            .unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            // Une fois les en-tetes recus, on laisse une derniere fenetre
            // courte pour le corps (chunked) avant de considerer la requete
            // complete.
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                let n2 = tokio::time::timeout(
                    std::time::Duration::from_millis(200),
                    stream.read(&mut chunk),
                )
                .await
                .unwrap_or(Ok(0))
                .unwrap_or(0);
                if n2 > 0 {
                    buf.extend_from_slice(&chunk[..n2]);
                }
                break;
            }
        }
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")
            .await;
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn disabled_auth_config() -> crate::auth::AuthConfig {
        crate::auth::AuthConfig {
            enabled: false,
            keycloak_url: String::new(),
            realm: String::new(),
            client_id: String::new(),
            super_admins: vec![],
            show_reset_button: false,
        }
    }

    #[tokio::test]
    async fn proxy_end_to_end_preserves_query_headers_method_and_body() {
        // 1. Fausse cible reelle (capture la requete brute recue).
        let (target_ready_tx, target_ready_rx) = tokio::sync::oneshot::channel();
        let target_server = tokio::spawn(capture_one_raw_request(target_ready_tx));
        let target_port = target_ready_rx.await.unwrap();

        // 2. Service en mode proxy pur (is_mocked=false : do_proxy direct,
        // sans passer par l'evaluation des regles).
        let data_dir = temp_dir_for_intercept_test();
        let store = MockStore::new(data_dir.join("mock-config.yaml"));
        store
            .replace(MockConfig {
                services: vec![Service {
                    name: "upstream".into(),
                    listen_path: "".into(),
                    real_target_url: format!("http://127.0.0.1:{target_port}"),
                    is_mocked: false,
                    rewrite_directory_urls: false,
                    group_name: None,
                    wsdl_mode: WsdlMode::default(),
                    rules: vec![],
                }],
                groups: vec![],
            })
            .await
            .unwrap();
        store.flush().await;

        // 3. Vrai serveur Axum complet (meme routeur qu'en production).
        #[cfg(feature = "messaging-kafka")]
        let messaging = crate::messaging::MessagingState {
            message_log: crate::messaging::message_log::MessageLog::new(),
            reply_topic: None,
            publisher: crate::messaging::consumer::Publisher::None,
        };
        let state = AppState {
            store,
            proxy: crate::engine::ProxyClient::new(),
            seq_counters: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
            request_log: crate::server::request_log::RequestLog::new(),
            auth_config: disabled_auth_config(),
            keycloak: None,
            script_engine: ScriptEngine::new(),
            ping_cache: crate::server::ping::PingCache::new(),
            #[cfg(feature = "messaging-kafka")]
            messaging,
        };
        let request_log_handle = state.request_log.clone();
        let app = crate::server::build_router(state, &data_dir);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // 4. Requete reelle avec query params + header custom + methode POST
        // + corps, envoyee au serveur lightMock (pas directement a la cible).
        let client = reqwest::Client::new();
        let resp = client
            .post(format!(
                "http://127.0.0.1:{server_port}/upstream/foo/bar?a=1&b=two"
            ))
            .header("x-custom-header", "custom-value")
            .body("payload-body")
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success() || resp.status().as_u16() == 200);

        let raw = target_server.await.unwrap();
        let request_line = raw.lines().next().unwrap_or("");
        assert!(
            request_line.starts_with("POST "),
            "methode HTTP non preservee: {request_line}"
        );
        assert!(
            request_line.contains("/foo/bar?a=1&b=two"),
            "query params/chemin non preserves dans la requete proxifiee: {request_line}"
        );
        assert!(
            raw.to_lowercase().contains("x-custom-header: custom-value"),
            "header custom manquant dans la requete proxifiee:\n{raw}"
        );
        assert!(
            raw.contains("payload-body"),
            "corps de requete manquant dans la requete proxifiee:\n{raw}"
        );

        // Proxy NIVEAU SERVICE : aucun RequestData n'est construit sur ce
        // chemin (streaming zero-buffering), donc aucun
        // detail n'est capturable pour le testeur de regle.
        let logged = request_log_handle.recent(1);
        assert_eq!(logged.len(), 1);
        assert!(logged[0].captured.is_none());

        std::fs::remove_dir_all(&data_dir).ok();
    }

    #[tokio::test]
    async fn rule_level_proxy_action_preserves_query_headers_method_and_body() {
        // Meme verification que le test precedent, mais pour l'AUTRE chemin
        // de code proxy : service is_mocked=true avec une regle
        // action=proxy, qui passe par rebuild_request_for_proxy() plutot que
        // par le Request original tel quel. Les deux chemins doivent se
        // comporter de facon identique du point de vue de la cible.
        let (target_ready_tx, target_ready_rx) = tokio::sync::oneshot::channel();
        let target_server = tokio::spawn(capture_one_raw_request(target_ready_tx));
        let target_port = target_ready_rx.await.unwrap();

        let data_dir = temp_dir_for_intercept_test();
        let store = MockStore::new(data_dir.join("mock-config.yaml"));
        store
            .replace(MockConfig {
                services: vec![Service {
                    name: "upstream2".into(),
                    listen_path: "".into(),
                    real_target_url: format!("http://127.0.0.1:{target_port}"),
                    is_mocked: true,
                    rewrite_directory_urls: false,
                    group_name: None,
                    wsdl_mode: WsdlMode::default(),
                    rules: vec![Rule {
                        name: "proxy-rule".into(),
                        method: "POST".into(),
                        sub_path: None,
                        action: RuleAction::Proxy,
                        pre_script: None,
                        script: None,
                        post_script: None,
                        conditions: ConditionGroup::default(),
                        response: MockResponse {
                            status: 200,
                            headers: vec![],
                            body: vec![],
                            chaos: None,
                        },
                    }],
                }],
                groups: vec![],
            })
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
            seq_counters: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
            request_log: crate::server::request_log::RequestLog::new(),
            auth_config: disabled_auth_config(),
            keycloak: None,
            script_engine: ScriptEngine::new(),
            ping_cache: crate::server::ping::PingCache::new(),
            #[cfg(feature = "messaging-kafka")]
            messaging,
        };
        let request_log_handle = state.request_log.clone();
        let app = crate::server::build_router(state, &data_dir);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(format!(
                "http://127.0.0.1:{server_port}/upstream2/foo/bar?a=1&b=two"
            ))
            .header("x-custom-header", "custom-value")
            .body("payload-body")
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success() || resp.status().as_u16() == 200);

        let raw = target_server.await.unwrap();
        let request_line = raw.lines().next().unwrap_or("");
        assert!(
            request_line.starts_with("POST "),
            "methode HTTP non preservee (rule-level proxy): {request_line}"
        );
        assert!(
            request_line.contains("/foo/bar?a=1&b=two"),
            "query params/chemin non preserves (rule-level proxy): {request_line}"
        );
        assert!(
            raw.to_lowercase().contains("x-custom-header: custom-value"),
            "header custom manquant (rule-level proxy):\n{raw}"
        );
        assert!(
            raw.contains("payload-body"),
            "corps de requete manquant (rule-level proxy):\n{raw}"
        );

        // Proxy NIVEAU REGLE : RequestData est deja bufferise pour evaluer les
        // regles avant ce branchement, donc le detail EST capturable ici,
        // contrairement au proxy niveau service ci-dessus.
        let logged = request_log_handle.recent(1);
        assert_eq!(logged.len(), 1);
        let captured = logged[0].captured.as_ref().expect("rule-level proxy doit capturer le detail de la requete");
        assert_eq!(captured.query_params.get("a").unwrap(), "1");
        assert_eq!(captured.headers.get("x-custom-header").unwrap(), "custom-value");
        assert_eq!(captured.body, "payload-body");

        std::fs::remove_dir_all(&data_dir).ok();
    }

    #[tokio::test]
    async fn mock_response_captures_request_detail_in_log() {
        // Verifie que le chemin mock (pas seulement proxy) capture bien le
        // detail de la requete pour le testeur de regle.
        let data_dir = temp_dir_for_intercept_test();
        let store = MockStore::new(data_dir.join("mock-config.yaml"));
        store
            .replace(MockConfig {
                services: vec![Service {
                    name: "mocksvc".into(),
                    listen_path: "/{id}/*".into(),
                    real_target_url: "http://unused.invalid".into(),
                    is_mocked: true,
                    rewrite_directory_urls: false,
                    group_name: None,
                    wsdl_mode: WsdlMode::default(),
                    rules: vec![Rule {
                        name: "mock-rule".into(),
                        method: "GET".into(),
                        sub_path: None,
                        action: RuleAction::Mock,
                        pre_script: None,
                        script: None,
                        post_script: None,
                        conditions: ConditionGroup::default(),
                        response: MockResponse {
                            status: 200,
                            headers: vec![],
                            body: vec![BodyFragment::Literal { value: "ok".into() }],
                            chaos: None,
                        },
                    }],
                }],
                groups: vec![],
            })
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
            seq_counters: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
            request_log: crate::server::request_log::RequestLog::new(),
            auth_config: disabled_auth_config(),
            keycloak: None,
            script_engine: ScriptEngine::new(),
            ping_cache: crate::server::ping::PingCache::new(),
            #[cfg(feature = "messaging-kafka")]
            messaging,
        };
        let request_log_handle = state.request_log.clone();
        let app = crate::server::build_router(state, &data_dir);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{server_port}/mocksvc/42/rest?foo=bar"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);

        let logged = request_log_handle.recent(1);
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].mode, "mock");
        let captured = logged[0].captured.as_ref().expect("le mock doit capturer le detail de la requete");
        assert_eq!(captured.path_params.get("id").unwrap(), "42");
        assert_eq!(captured.query_params.get("foo").unwrap(), "bar");
        assert!(!captured.body_truncated);

        std::fs::remove_dir_all(&data_dir).ok();
    }
}
