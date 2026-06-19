use crate::engine::{apply_chaos_and_render, MatchEngine, RequestData, TemplateContext};
use crate::models::Service;
use crate::server::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

pub async fn intercept_layer(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    if path.starts_with("/api/") || path == "/api" {
        return next.run(req).await;
    }

    let config = state.store.snapshot().await;

    let matched = config.services.iter().find_map(|s| {
        if method.as_str() != s.method {
            return None;
        }
        let effective = build_effective_pattern(&s.name, &s.listen_path);
        match_path(&effective, &path).map(|(params, remaining)| (s.clone(), params, remaining))
    });

    match matched {
        Some((service, path_params, remaining)) => {
            handle_service(&state, &service, &path, path_params, &remaining, req).await
        }
        None => next.run(req).await,
    }
}

fn build_effective_pattern(name: &str, listen_path: &str) -> String {
    let lp = listen_path.trim_start_matches('/');
    format!("/{name}/{lp}")
}

async fn handle_service(
    state: &AppState,
    service: &Service,
    path: &str,
    path_params: HashMap<String, String>,
    remaining: &str,
    req: Request<Body>,
) -> Response {
    let method_str = req.method().to_string();

    if !service.is_mocked {
        let target = format!(
            "{}/{}",
            service.real_target_url.trim_end_matches('/'),
            remaining.trim_start_matches('/')
        );
        tracing::info!(
            service_key = %service.name,
            method = %method_str,
            path = %path,
            mode = "proxy",
            target = %target,
            "proxy forwarding"
        );
        return match state.proxy.forward(&service.real_target_url, remaining, req).await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                state.request_log.log_proxy(&service.name, &method_str, path, &target, status);
                resp
            }
            Err(status) => {
                state.request_log.log_proxy(&service.name, &method_str, path, &target, status.as_u16());
                status.into_response()
            }
        };
    }

    let query_params = extract_query_params(req.uri().query());
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
    };

    let matched_rule = MatchEngine::first_match(&service.rules, &request_data);

    let Some(rule) = matched_rule else {
        tracing::warn!(
            service_key = %service.name, method = %method_str, path = %path,
            mode = "no-rule",
            "no matching rule"
        );
        state.request_log.log_no_rule(&service.name, &method_str, path);
        return StatusCode::NOT_FOUND.into_response();
    };

    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let seq = state.next_seq(&service.name);

    let ctx = TemplateContext {
        path_params: &path_params,
        query_params: &request_data.query_params,
        headers: &request_data.headers,
        request_body: &request_data.body,
        seq_counter: seq,
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
            state.request_log.log_mock(&service.name, &method_str, path, &rule.name, status.as_u16());
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
            state.request_log.log_mock(&service.name, &method_str, path, &rule.name, status.as_u16());
            (status, "chaos error injected").into_response()
        }
    }
}

fn match_path(listen_path: &str, request_path: &str) -> Option<(HashMap<String, String>, String)> {
    let pattern_str = normalize_colon_syntax(listen_path);

    let pattern_segs: Vec<&str> = pattern_str.split('/').filter(|s| !s.is_empty()).collect();
    let request_segs: Vec<&str> = request_path.split('/').filter(|s| !s.is_empty()).collect();

    if pattern_segs.is_empty() {
        return Some((HashMap::new(), request_path.to_string()));
    }

    let mut params = HashMap::new();
    let mut has_wildcard = false;
    let mut matched_count = 0;

    for (i, pat) in pattern_segs.iter().enumerate() {
        if *pat == "*" {
            has_wildcard = true;
            matched_count = i;
            break;
        }
        if i >= request_segs.len() {
            return None;
        }
        if pat.starts_with('{') && pat.ends_with('}') {
            let name = &pat[1..pat.len() - 1];
            params.insert(name.to_string(), request_segs[i].to_string());
        } else if *pat != request_segs[i] {
            return None;
        }
        matched_count = i + 1;
    }

    if !has_wildcard && request_segs.len() != pattern_segs.len() {
        return None;
    }

    let remaining = if matched_count < request_segs.len() {
        format!("/{}", request_segs[matched_count..].join("/"))
    } else {
        String::new()
    };

    Some((params, remaining))
}

fn normalize_colon_syntax(s: &str) -> String {
    s.split('/')
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':') {
                format!("{{{name}}}")
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
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

    #[test]
    fn effective_pattern() {
        assert_eq!(
            build_effective_pattern("insee", "/v4/sirene/{siret}"),
            "/insee/v4/sirene/{siret}"
        );
        assert_eq!(
            build_effective_pattern("svc-a", "/*"),
            "/svc-a/*"
        );
    }

    #[test]
    fn namespace_match() {
        let pattern = build_effective_pattern("insee", "/v4/sirene/{siret}");
        let r = match_path(&pattern, "/insee/v4/sirene/44306184100047");
        assert!(r.is_some());
        let (params, remaining) = r.unwrap();
        assert_eq!(params.get("siret").unwrap(), "44306184100047");
        assert_eq!(remaining, "");
    }

    #[test]
    fn namespace_no_collision() {
        let p1 = build_effective_pattern("svc-a", "/users/*");
        let p2 = build_effective_pattern("svc-b", "/users/*");
        assert!(match_path(&p1, "/svc-a/users/42").is_some());
        assert!(match_path(&p1, "/svc-b/users/42").is_none());
        assert!(match_path(&p2, "/svc-b/users/42").is_some());
    }

    #[test]
    fn namespace_wildcard_remaining() {
        let pattern = build_effective_pattern("api", "/*");
        let r = match_path(&pattern, "/api/foo/bar");
        assert!(r.is_some());
        let (_, remaining) = r.unwrap();
        assert_eq!(remaining, "/foo/bar");
    }

    #[test]
    fn wildcard_backward_compat() {
        let r = match_path("/svc-a/*", "/svc-a/foo/bar");
        assert!(r.is_some());
        let (params, remaining) = r.unwrap();
        assert!(params.is_empty());
        assert_eq!(remaining, "/foo/bar");
    }

    #[test]
    fn named_param_single() {
        let r = match_path("/v4/insee/{siret}", "/v4/insee/44306184100047");
        assert!(r.is_some());
        let (params, _) = r.unwrap();
        assert_eq!(params.get("siret").unwrap(), "44306184100047");
    }

    #[test]
    fn named_param_no_match_extra() {
        assert!(match_path("/v4/insee/{siret}", "/v4/insee/123/extra").is_none());
    }

    #[test]
    fn colon_syntax_normalized() {
        let r = match_path("/api/:version/users/:id", "/api/v2/users/42");
        assert!(r.is_some());
        let (params, _) = r.unwrap();
        assert_eq!(params.get("version").unwrap(), "v2");
        assert_eq!(params.get("id").unwrap(), "42");
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
}
