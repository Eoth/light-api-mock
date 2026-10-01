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
use crate::server::validation::is_management_api_route;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

/// Origins allowed to call the management API from a browser, besides the UI's own origin.
#[derive(Debug, Clone, Default)]
pub struct BrowserGuard {
    allowed_origins: Arc<Vec<HeaderValue>>,
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
        }
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

    fn refuses(&self, method: &Method, path: &str, headers: &HeaderMap) -> bool {
        if !is_management_api_route(path) || is_safe_method(method) {
            return false;
        }
        let cross_site = matches!(
            headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()),
            Some("cross-site") | Some("same-site")
        );
        cross_site && !headers.get("origin").is_some_and(|o| self.allows(o))
    }
}

fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

pub async fn cross_site_write_guard(
    State(guard): State<BrowserGuard>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if guard.refuses(req.method(), req.uri().path(), req.headers()) {
        tracing::warn!(
            method = %req.method(),
            path = %req.uri().path(),
            origin = ?req.headers().get("origin"),
            "cross-site write to the management API refused"
        );
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "Cross-site request refused: add this origin to CORS_ALLOWED_ORIGINS to allow it."
            })),
        )
            .into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests;
