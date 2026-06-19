use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use reqwest::Client;

#[derive(Clone)]
pub struct ProxyClient {
    client: Client,
}

impl ProxyClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("reqwest client"),
        }
    }

    pub async fn forward(
        &self,
        target_base: &str,
        remaining_path: &str,
        req: Request<Body>,
    ) -> Result<Response<Body>, StatusCode> {
        let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
        let url = format!(
            "{}/{}{}",
            target_base.trim_end_matches('/'),
            remaining_path.trim_start_matches('/'),
            query,
        );

        let method = req.method().clone();
        let mut builder = self.client.request(
            reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap(),
            &url,
        );

        let original_host = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        for (name, value) in req.headers() {
            let name_str = name.as_str();
            if is_hop_by_hop(name_str) {
                continue;
            }
            if let Ok(v) = value.to_str() {
                builder = builder.header(name_str, v);
            }
        }

        if !original_host.is_empty() {
            builder = builder.header("X-Forwarded-Host", &original_host);
        }
        builder = builder.header("X-Forwarded-Proto", "http");

        let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        if !body_bytes.is_empty() {
            builder = builder.body(body_bytes.to_vec());
        }

        let upstream_resp = builder.send().await.map_err(|e| {
            tracing::error!(error = %e, url = %url, "proxy forward failed");
            StatusCode::BAD_GATEWAY
        })?;

        let status = StatusCode::from_u16(upstream_resp.status().as_u16())
            .unwrap_or(StatusCode::BAD_GATEWAY);

        let mut response = Response::builder().status(status);
        for (name, value) in upstream_resp.headers() {
            let name_str = name.as_str();
            if is_hop_by_hop(name_str) {
                continue;
            }
            if let Ok(v) = value.to_str() {
                response = response.header(name_str, v);
            }
        }

        let resp_bytes = upstream_resp.bytes().await.map_err(|e| {
            tracing::error!(error = %e, "proxy read body failed");
            StatusCode::BAD_GATEWAY
        })?;

        response
            .body(Body::from(resp_bytes))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name,
        "host"
            | "connection"
            | "transfer-encoding"
            | "keep-alive"
            | "te"
            | "trailers"
            | "upgrade"
            | "proxy-authorization"
            | "proxy-connection"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_client_creates_successfully() {
        let _client = ProxyClient::new();
    }

    #[tokio::test]
    async fn forward_to_invalid_host_returns_bad_gateway() {
        let client = ProxyClient::new();
        let req = Request::builder()
            .method("GET")
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let result = client
            .forward("http://127.0.0.1:1", "/test", req)
            .await;
        assert_eq!(result.unwrap_err(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn url_construction() {
        let base = "http://svc.default.svc:8080/";
        let remaining = "/api/users";
        let combined = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            remaining.trim_start_matches('/'),
        );
        assert_eq!(combined, "http://svc.default.svc:8080/api/users");
    }

    #[test]
    fn url_construction_no_trailing_slash() {
        let base = "http://svc:8080";
        let remaining = "api/v1";
        let combined = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            remaining.trim_start_matches('/'),
        );
        assert_eq!(combined, "http://svc:8080/api/v1");
    }
}
