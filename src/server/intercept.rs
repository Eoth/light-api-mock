use crate::engine::matcher::match_path;
use crate::engine::script::{ScriptContext, ScriptResult};
use crate::engine::{apply_chaos_and_render, MatchEngine, RequestData, TemplateContext};
use crate::models::{RuleAction, Service, WsdlMode};
use crate::server::AppState;
use crate::server::validation::is_internal_route;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

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
                .log_proxy(&service.name, method_str, path, &target, status);
            resp
        }
        Err(status) => {
            state
                .request_log
                .log_proxy(&service.name, method_str, path, &target, status.as_u16());
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
        return do_proxy(state, service, path, &method_str, "service-level", gc, req).await;
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
                return do_proxy(state, service, path, &method_str, "wsdl-bypass", gc, req).await;
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

    let matched = MatchEngine::first_match(&service.rules, &request_data);

    let Some((rule, sub_params)) = matched else {
        tracing::warn!(
            service_key = %service.name, method = %method_str, path = %path,
            mode = "no-rule",
            "no matching rule, returning 404"
        );
        state
            .request_log
            .log_no_rule(&service.name, &method_str, path);
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
        )
        .await;
    }

    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let seq = state.next_seq(&service.name);

    let mut merged_params = path_params;
    merged_params.extend(sub_params);

    let script_result = if let Some(ref script) = rule.script {
        let script_ctx = ScriptContext {
            body: String::from_utf8_lossy(&request_data.body).into_owned(),
            headers: request_data.headers.clone(),
            query_params: request_data.query_params.clone(),
            path_params: merged_params.clone(),
        };
        match state.script_engine.execute(script, &script_ctx) {
            Ok(result) => Some(result),
            Err(e) => {
                tracing::warn!(rule = %rule.name, error = %e, "script execution failed");
                Some(ScriptResult::default())
            }
        }
    } else {
        None
    };

    let ctx = TemplateContext {
        path_params: &merged_params,
        query_params: &request_data.query_params,
        headers: &request_data.headers,
        request_body: &request_data.body,
        seq_counter: seq,
        script_result: script_result.as_ref(),
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
}
