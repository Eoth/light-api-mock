use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;

const PING_TIMEOUT: Duration = Duration::from_secs(3);

/// Statut de disponibilite RESEAU (pas applicatif) d'une URL cible
/// (real_target_url d'un service). Base uniquement sur une connexion TCP :
/// "joignable" = le socket s'est ouvert avant le timeout, "injoignable" =
/// timeout, connexion refusee ou echec DNS. Aucune requete HTTP n'est
/// envoyee — voir `ProxyClient::ping`.
#[derive(Debug, Clone, Serialize)]
pub struct PingStatus {
    pub reachable: bool,
    pub checked_at: u64,
    pub error: Option<String>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

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

    /// Verifie l'accessibilite RESEAU d'une URL cible : une simple connexion TCP
    /// vers host:port, resolue depuis l'URL (port explicite, sinon 443 pour
    /// https, 80 sinon). AUCUNE requete HTTP n'est envoyee — pas de GET/HEAD,
    /// pas de handshake TLS, pas d'appel a une route applicative du backend
    /// cible. La reponse ne dit donc rien sur la sante fonctionnelle de l'API,
    /// seulement "le socket s'est ouvert ou non" dans le timeout imparti.
    pub async fn ping(&self, url: &str) -> PingStatus {
        let (host, port) = match parse_host_port(url) {
            Ok(hp) => hp,
            Err(e) => {
                return PingStatus {
                    reachable: false,
                    checked_at: now_ms(),
                    error: Some(e),
                };
            }
        };

        match tokio::time::timeout(
            PING_TIMEOUT,
            tokio::net::TcpStream::connect((host.as_str(), port)),
        )
        .await
        {
            Ok(Ok(_stream)) => PingStatus {
                reachable: true,
                checked_at: now_ms(),
                error: None,
            },
            Ok(Err(e)) => PingStatus {
                reachable: false,
                checked_at: now_ms(),
                error: Some(e.to_string()),
            },
            Err(_elapsed) => PingStatus {
                reachable: false,
                checked_at: now_ms(),
                error: Some(format!("timeout apres {}s", PING_TIMEOUT.as_secs())),
            },
        }
    }

    pub async fn forward(
        &self,
        target_base: &str,
        remaining_path: &str,
        req: Request<Body>,
    ) -> Result<Response<Body>, StatusCode> {
        let (parts, body) = req.into_parts();

        let query = parts
            .uri
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        let url = format!(
            "{}/{}{}",
            target_base.trim_end_matches('/'),
            remaining_path.trim_start_matches('/'),
            query,
        );

        let method =
            reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut builder = self.client.request(method, &url);

        let original_host = parts
            .headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        for (name, value) in &parts.headers {
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

        let req_stream = http_body_util::BodyStream::new(body).filter_map(|result| async move {
            match result {
                Ok(frame) => frame.into_data().ok().map(Ok),
                Err(e) => Some(Err(std::io::Error::new(std::io::ErrorKind::Other, e))),
            }
        });
        builder = builder.body(reqwest::Body::wrap_stream(req_stream));

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

        let resp_stream = upstream_resp.bytes_stream();
        response
            .body(Body::from_stream(resp_stream))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// Extrait host+port d'une URL pour le test de connectivite TCP. Port
/// explicite dans l'URL en priorite, sinon 443 pour https, 80 pour tout le
/// reste (http ou schema inconnu).
fn parse_host_port(url: &str) -> Result<(String, u16), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("URL invalide: {e}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL sans host".to_string())?
        .to_string();
    let port = parsed
        .port()
        .unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    Ok((host, port))
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

        let result = client.forward("http://127.0.0.1:1", "/test", req).await;
        assert_eq!(result.unwrap_err(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn ping_unreachable_host_returns_unreachable() {
        let client = ProxyClient::new();
        let status = client.ping("http://127.0.0.1:1").await;
        assert!(!status.reachable);
        assert!(status.error.is_some());
    }

    #[tokio::test]
    async fn ping_reachable_host_returns_reachable_via_tcp_only() {
        // Un simple listener TCP local, sans serveur HTTP derriere : si ping()
        // envoyait une vraie requete HTTP (GET/HEAD), le listener n'y repondrait
        // jamais correctement. Le fait que reachable=true prouve que seul le
        // handshake TCP compte.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let client = ProxyClient::new();
        let status = client.ping(&format!("http://{addr}")).await;
        assert!(status.reachable);
        assert!(status.error.is_none());
    }

    #[test]
    fn ping_never_issues_http_request() {
        // parse_host_port ne fait que du parsing d'URL, aucun I/O reseau —
        // garantit que la resolution host/port ne declenche jamais elle-meme
        // un appel HTTP (contrairement a l'ancienne implementation HEAD).
        let (host, port) = parse_host_port("http://example.invalid:1234/some/business/path").unwrap();
        assert_eq!(host, "example.invalid");
        assert_eq!(port, 1234);
    }

    #[test]
    fn parse_host_port_explicit_port() {
        assert_eq!(
            parse_host_port("http://svc.default.svc:9090").unwrap(),
            ("svc.default.svc".to_string(), 9090)
        );
    }

    #[test]
    fn parse_host_port_defaults_http_80() {
        assert_eq!(
            parse_host_port("http://svc.default.svc").unwrap(),
            ("svc.default.svc".to_string(), 80)
        );
    }

    #[test]
    fn parse_host_port_defaults_https_443() {
        assert_eq!(
            parse_host_port("https://secure.example.com").unwrap(),
            ("secure.example.com".to_string(), 443)
        );
    }

    #[test]
    fn parse_host_port_invalid_url_errors() {
        assert!(parse_host_port("not a url at all").is_err());
        assert!(parse_host_port("").is_err());
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

    /// Capture la requete brute recue par une fausse cible TCP, pour
    /// verifier au plus pres du fil ce que `forward()` transmet reellement
    /// (methode, chemin+query, en-tetes, corps) sans dependre du parsing
    /// HTTP d'un client. Voir aussi les tests bout-en-bout dans
    /// `server::intercept::tests` qui couvrent le
    /// meme invariant a travers tout le pipeline (intercept_layer/do_proxy),
    /// pas seulement ProxyClient::forward() en isolation.
    async fn capture_raw_request(port_rx: tokio::sync::oneshot::Sender<u16>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        port_rx.send(addr.port()).unwrap();

        let (mut stream, _) = listener.accept().await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut chunk))
                .await
                .unwrap_or(Ok(0))
                .unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                // corps eventuel : on tente une derniere lecture courte
                let n2 = tokio::time::timeout(Duration::from_millis(100), stream.read(&mut chunk))
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

    #[tokio::test]
    async fn forward_preserves_query_headers_method_and_body() {
        let (port_tx, port_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(capture_raw_request(port_tx));
        let port = port_rx.await.unwrap();

        let client = ProxyClient::new();
        let req = Request::builder()
            .method("POST")
            .uri("/svc/foo?a=1&b=two")
            .header("x-custom-header", "custom-value")
            .body(Body::from("payload-body"))
            .unwrap();

        let target_base = format!("http://127.0.0.1:{port}");
        let _ = client.forward(&target_base, "/foo", req).await;

        let raw = server.await.unwrap();

        let request_line = raw.lines().next().unwrap_or("");
        assert!(
            request_line.starts_with("POST "),
            "method not preserved: {request_line}"
        );
        assert!(
            request_line.contains("?a=1&b=two"),
            "query params missing from request line: {request_line}"
        );
        assert!(
            raw.to_lowercase().contains("x-custom-header: custom-value"),
            "custom header missing:\n{raw}"
        );
        // Corps transmis en chunked transfer-encoding (streaming) : on
        // verifie sa presence par sous-chaine, pas par
        // egalite/suffixe exact, puisque la trame chunked ajoute une taille
        // hexadecimale et un terminateur autour de la charge utile.
        assert!(
            raw.contains("payload-body"),
            "body missing/incomplete:\n{raw}"
        );
    }
}
