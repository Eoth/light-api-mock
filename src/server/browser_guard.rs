//! Browser-facing protections of the management API (`/api/...`).
//!
//! Any website open in a browser can send requests to a lightMock reachable from that machine (often `localhost`,
//! often without authentication during development). These rules keep such a page from reading or changing the
//! configuration, while the mocked and proxied services stay callable from any origin, which is their purpose:
//! - CORS: the management API answers cross-origin calls only for the origins listed in `CORS_ALLOWED_ORIGINS`
//!   (none by default; only needed when the UI is served from another origin, see `API_BASE_URL`).
//! - Cross-site writes: a state-changing API request that the browser itself marks as coming from another site
//!   (`Sec-Fetch-Site`) is refused unless its `Origin` is listed. This also covers the requests that a browser
//!   sends without a CORS preflight (a form `POST`, for instance).
//! - DNS rebinding: when lightMock only listens on the loopback interface (the default of the binary), a page can
//!   still reach it by making its own domain name resolve to 127.0.0.1; the browser then treats the calls as
//!   same-origin. The management API therefore answers only requests addressed to a loopback name (`localhost`,
//!   `127.0.0.1`, `[::1]`) in that case.
use crate::server::validation::is_management_api_route;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

#[derive(Debug, Clone, Default)]
pub struct BrowserGuard {
    /// Origins allowed to call the management API from a browser, besides the UI's own origin.
    allowed_origins: Arc<Vec<HeaderValue>>,
    /// Set when the server only listens on a loopback address.
    loopback_hosts_only: bool,
}

impl BrowserGuard {
    pub fn from_env() -> Self {
        Self::new(&std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default())
    }

    /// `raw` is a comma-separated list of origins (`https://ui.example.com,http://localhost:5173`); a trailing slash
    /// is ignored because browsers never send one in `Origin`.
    pub fn new(raw: &str) -> Self {
        let origins = raw
            .split(',')
            .map(|o| o.trim().trim_end_matches('/'))
            .filter(|o| !o.is_empty())
            .filter_map(|o| match HeaderValue::from_str(o) {
                Ok(v) => Some(v),
                Err(_) => {
                    tracing::warn!(
                        origin = o,
                        "CORS_ALLOWED_ORIGINS: ignoring an invalid origin"
                    );
                    None
                }
            })
            .collect();
        Self {
            allowed_origins: Arc::new(origins),
            loopback_hosts_only: false,
        }
    }

    pub fn with_loopback_hosts_only(mut self, enabled: bool) -> Self {
        self.loopback_hosts_only = enabled;
        self
    }

    fn allows(&self, origin: &HeaderValue) -> bool {
        self.allowed_origins.iter().any(|o| o == origin)
    }

    pub fn cors_layer(&self) -> CorsLayer {
        let guard = self.clone();
        CorsLayer::new()
            .allow_origin(AllowOrigin::predicate(move |origin, parts| {
                !is_management_api_route(parts.uri.path()) || guard.allows(origin)
            }))
            .allow_methods(Any)
            .allow_headers(Any)
    }

    /// Why the request must be refused, if it must.
    fn refusal(&self, method: &Method, path: &str, headers: &HeaderMap) -> Option<&'static str> {
        if !is_management_api_route(path) {
            return None;
        }
        if self.loopback_hosts_only
            && headers
                .get("host")
                .and_then(|h| h.to_str().ok())
                .is_some_and(|h| !is_loopback_host(h))
        {
            return Some(
                "Request refused: this lightMock only listens on the local machine; call it through localhost.",
            );
        }
        let cross_site = matches!(
            headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()),
            Some("cross-site") | Some("same-site")
        );
        if !is_safe_method(method)
            && cross_site
            && !headers.get("origin").is_some_and(|o| self.allows(o))
        {
            return Some(
                "Cross-site request refused: add this origin to CORS_ALLOWED_ORIGINS to allow it.",
            );
        }
        None
    }
}

fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// `host` is a `Host` header value: a name or an IP address, with an optional port (`[::1]:7342`).
fn is_loopback_host(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(""),
        None => host.rsplit_once(':').map_or(host, |(name, _port)| name),
    }
    .to_ascii_lowercase();
    name == "localhost"
        || name.ends_with(".localhost")
        || name
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

pub async fn management_api_guard(
    State(guard): State<BrowserGuard>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if let Some(reason) = guard.refusal(req.method(), req.uri().path(), req.headers()) {
        tracing::warn!(
            method = %req.method(),
            path = %req.uri().path(),
            origin = ?req.headers().get("origin"),
            host = ?req.headers().get("host"),
            "management API request refused by the browser guard"
        );
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({ "error": reason })),
        )
            .into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests;
