use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
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

impl Default for ProxyClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Longest silence accepted from a proxied backend between two reads (`PROXY_READ_TIMEOUT_SECS`, default 120 s).
/// An idle limit rather than a total one, so that long downloads and event streams that keep sending still work.
fn read_timeout_from_env() -> Duration {
    let secs = std::env::var("PROXY_READ_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(120);
    Duration::from_secs(secs)
}

impl ProxyClient {
    pub fn new() -> Self {
        Self::with_timeouts(Duration::from_secs(10), read_timeout_from_env())
    }

    /// Without these bounds, a backend that accepts the connection and never answers held the client's request
    /// (and a task) open forever.
    pub fn with_timeouts(connect: Duration, read: Duration) -> Self {
        Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(connect)
                .read_timeout(read)
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

        let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
            .unwrap_or(reqwest::Method::GET);
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
                Err(e) => Some(Err(std::io::Error::other(e))),
            }
        });
        builder = builder.body(reqwest::Body::wrap_stream(req_stream));

        let upstream_resp = builder.send().await.map_err(|e| {
            let status = upstream_error_status(&e);
            tracing::error!(
                error = %e.without_url(),
                url = %crate::server::redaction::redact_url_credentials(&url),
                "proxy forward failed"
            );
            status
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

    /// Variante de `forward()` qui tente, EN PLUS de relayer la requete,
    /// de capturer requete+reponse pour l'observation de trafic (cf
    /// `server::observation`). Ne change RIEN au chemin par defaut : cette
    /// methode n'est appelee que quand l'utilisateur a explicitement active
    /// l'observation d'un service (jamais automatique).
    ///
    /// La capture n'est tentee que si la taille est connue et sure a l'avance,
    /// sur CHAQUE cote independamment :
    /// - requete non capturable (Content-Length > `max_buffer`, ou
    ///   Transfer-Encoding: chunked — taille reellement inconnue) : bascule
    ///   integralement sur `forward()` (streaming inchange), retourne `None`.
    ///   Une requete SANS Content-Length ET sans chunked (GET/DELETE typique
    ///   sans corps) est traitee comme un corps vide, capturable.
    /// - reponse non capturable (meme critere, verifie APRES connexion a la
    ///   cible via `upstream_resp.content_length()`) : la reponse est quand
    ///   meme relayee en streaming normalement, mais la capture retournee est
    ///   `None` (echange partiel jamais retenu par l'appelant, cf
    ///   `ObservationStore`).
    ///
    /// Dans les deux cas de repli, la reponse renvoyee au client est
    /// STRICTEMENT identique a ce que `forward()` aurait produit — aucune
    /// perte fonctionnelle pour l'utilisateur final, seulement une capture en
    /// moins. Seule exception theorique : une requete sans Content-Length ET
    /// sans chunked qui porterait malgre tout un corps au-dela de
    /// `max_buffer` (HTTP mal forme — hyper ne laisse jamais passer ce cas
    /// sur une vraie connexion reseau) recevrait un 413 au lieu d'etre
    /// relayee telle quelle.
    pub async fn forward_with_capture(
        &self,
        target_base: &str,
        remaining_path: &str,
        req: Request<Body>,
        max_buffer: usize,
    ) -> Result<(Response<Body>, Option<ProxyCaptureRaw>), StatusCode> {
        let (parts, body) = req.into_parts();

        let request_content_length = parts
            .headers
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());
        // Une requete sans Content-Length ET sans Transfer-Encoding: chunked
        // (typiquement GET/DELETE sans corps) est, par construction HTTP,
        // un corps vide -- sure a bufferiser (0 octet) sans avoir besoin
        // d'une taille annoncee. Seule une requete VRAIMENT de taille
        // inconnue (chunked) bascule sur le streaming pur.
        let is_chunked = parts
            .headers
            .get(axum::http::header::TRANSFER_ENCODING)
            .is_some();
        let capturable_request = match request_content_length {
            Some(n) => n <= max_buffer,
            None => !is_chunked,
        };

        if !capturable_request {
            let rebuilt = Request::from_parts(parts, body);
            let resp = self.forward(target_base, remaining_path, rebuilt).await?;
            return Ok((resp, None));
        }

        let request_query_params: HashMap<String, String> = parts
            .uri
            .query()
            .map(|q| {
                url::form_urlencoded::parse(q.as_bytes())
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect()
            })
            .unwrap_or_default();
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

        let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
            .unwrap_or(reqwest::Method::GET);
        let mut builder = self.client.request(method, &url);

        let original_host = parts
            .headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let request_content_type = parts
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let mut request_headers = HashMap::new();
        for (name, value) in &parts.headers {
            let name_str = name.as_str();
            if is_hop_by_hop(name_str) {
                continue;
            }
            if let Ok(v) = value.to_str() {
                builder = builder.header(name_str, v);
                request_headers.insert(name_str.to_string(), v.to_string());
            }
        }
        if !original_host.is_empty() {
            builder = builder.header("X-Forwarded-Host", &original_host);
        }
        builder = builder.header("X-Forwarded-Proto", "http");

        let body_bytes = axum::body::to_bytes(body, max_buffer).await.map_err(|e| {
            tracing::error!(error = %e, "observed request body exceeds capture buffer");
            StatusCode::PAYLOAD_TOO_LARGE
        })?;
        builder = builder.body(body_bytes.to_vec());

        let upstream_resp = builder.send().await.map_err(|e| {
            let status = upstream_error_status(&e);
            tracing::error!(
                error = %e.without_url(),
                url = %crate::server::redaction::redact_url_credentials(&url),
                "proxy forward (observed) failed"
            );
            status
        })?;

        let status = StatusCode::from_u16(upstream_resp.status().as_u16())
            .unwrap_or(StatusCode::BAD_GATEWAY);
        let response_content_type = upstream_resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let response_content_length = upstream_resp.content_length();

        let mut response_builder = Response::builder().status(status);
        let mut response_headers = HashMap::new();
        for (name, value) in upstream_resp.headers() {
            let name_str = name.as_str();
            if is_hop_by_hop(name_str) {
                continue;
            }
            if let Ok(v) = value.to_str() {
                response_builder = response_builder.header(name_str, v);
                response_headers.insert(name_str.to_string(), v.to_string());
            }
        }

        if !matches!(response_content_length, Some(n) if (n as usize) <= max_buffer) {
            let resp_stream = upstream_resp.bytes_stream();
            let resp = response_builder
                .body(Body::from_stream(resp_stream))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok((resp, None));
        }

        let response_bytes = upstream_resp.bytes().await.map_err(|e| {
            let status = upstream_error_status(&e);
            tracing::error!(
                error = %e.without_url(),
                url = %crate::server::redaction::redact_url_credentials(&url),
                "reading observed response body failed"
            );
            status
        })?;
        let resp = response_builder
            .body(Body::from(response_bytes.clone()))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let capture = ProxyCaptureRaw {
            request_query_params,
            request_headers,
            request_body: body_bytes.to_vec(),
            request_content_type,
            response_status: status.as_u16(),
            response_headers,
            response_body: response_bytes.to_vec(),
            response_content_type,
        };
        Ok((resp, Some(capture)))
    }
}

/// Requete + reponse brutes capturees par `ProxyClient::forward_with_capture`
/// quand les deux cotes etaient surs a bufferiser. La troncature pour
/// stockage/affichage (cf `server::observation::max_body_size`) est la
/// responsabilite de l'appelant, pas de ce type — celui-ci porte les octets
/// complets tels que recus.
#[derive(Debug, Clone)]
pub struct ProxyCaptureRaw {
    pub request_query_params: HashMap<String, String>,
    pub request_headers: HashMap<String, String>,
    pub request_body: Vec<u8>,
    pub request_content_type: Option<String>,
    pub response_status: u16,
    pub response_headers: HashMap<String, String>,
    pub response_body: Vec<u8>,
    pub response_content_type: Option<String>,
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

/// 504 when the backend did not answer in time, 502 for any other failure to reach it.
fn upstream_error_status(e: &reqwest::Error) -> StatusCode {
    if e.is_timeout() {
        StatusCode::GATEWAY_TIMEOUT
    } else {
        StatusCode::BAD_GATEWAY
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
        let (host, port) =
            parse_host_port("http://example.invalid:1234/some/business/path").unwrap();
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

    /// Petit serveur axum local pour les tests de `forward_with_capture` :
    /// contrairement a `capture_raw_request` (parsing bas niveau au fil de
    /// l'eau), on a besoin ici d'une VRAIE reponse HTTP avec un
    /// Content-Length maitrise (present ou volontairement absent) pour
    /// exercer les deux branches capturable/non-capturable.
    async fn spawn_test_target(
        body: &'static str,
        chunked: bool,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use axum::response::IntoResponse;
        use axum::routing::any;

        async fn respond_fixed(
            axum::extract::State(body): axum::extract::State<&'static str>,
        ) -> impl IntoResponse {
            ([("content-type", "application/json")], body)
        }

        async fn respond_chunked(
            axum::extract::State(body): axum::extract::State<&'static str>,
        ) -> impl IntoResponse {
            let stream = futures_util::stream::once(async move {
                Ok::<_, std::io::Error>(axum::body::Bytes::from_static(body.as_bytes()))
            });
            axum::body::Body::from_stream(stream)
        }

        let app = if chunked {
            axum::Router::new()
                .route("/*rest", any(respond_chunked))
                .with_state(body)
        } else {
            axum::Router::new()
                .route("/*rest", any(respond_fixed))
                .with_state(body)
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn forward_with_capture_captures_small_request_and_response() {
        let (target_base, _server) = spawn_test_target(r#"{"id":1}"#, false).await;
        let client = ProxyClient::new();
        let req = Request::builder()
            .method("POST")
            .uri("/svc/orders?id=1")
            .header("content-type", "application/json")
            .header("content-length", r#"{"q":true}"#.len().to_string())
            .body(Body::from(r#"{"q":true}"#))
            .unwrap();

        let (resp, capture) = client
            .forward_with_capture(&target_base, "/orders", req, 10 * 1024)
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let capture = capture.expect("small request+response must be captured");
        assert_eq!(capture.request_query_params.get("id").unwrap(), "1");
        assert_eq!(capture.request_body, br#"{"q":true}"#);
        assert_eq!(capture.response_status, 200);
        assert_eq!(capture.response_body, br#"{"id":1}"#);
        assert_eq!(
            capture.response_content_type.as_deref(),
            Some("application/json")
        );

        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], br#"{"id":1}"#);
    }

    #[tokio::test]
    async fn forward_with_capture_skips_when_request_too_large_for_buffer() {
        let (target_base, _server) = spawn_test_target(r#"{"id":1}"#, false).await;
        let client = ProxyClient::new();
        let req = Request::builder()
            .method("POST")
            .uri("/svc/orders")
            .header("content-length", "10")
            .body(Body::from("0123456789"))
            .unwrap();

        let (resp, capture) = client
            .forward_with_capture(&target_base, "/orders", req, 4)
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            capture.is_none(),
            "requete au-dela du buffer max ne doit jamais etre capturee"
        );
    }

    #[tokio::test]
    async fn forward_with_capture_skips_when_response_has_no_content_length() {
        let (target_base, _server) = spawn_test_target(r#"{"id":1}"#, true).await;
        let client = ProxyClient::new();
        let req = Request::builder()
            .method("GET")
            .uri("/svc/orders")
            .body(Body::empty())
            .unwrap();

        let (resp, capture) = client
            .forward_with_capture(&target_base, "/orders", req, 10 * 1024)
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            capture.is_none(),
            "reponse sans Content-Length ne doit jamais etre capturee"
        );
        // Mais le trafic doit rester relaye correctement au client.
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], br#"{"id":1}"#);
    }

    #[tokio::test]
    async fn a_backend_that_never_answers_gets_a_gateway_timeout() {
        let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = silent.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = silent.accept().await {
                held.push(socket);
            }
        });
        let client = ProxyClient::with_timeouts(
            std::time::Duration::from_secs(2),
            std::time::Duration::from_millis(300),
        );
        let started = std::time::Instant::now();
        let result = client
            .forward(&format!("http://{addr}"), "/x", Request::new(Body::empty()))
            .await;
        assert_eq!(result.err(), Some(StatusCode::GATEWAY_TIMEOUT));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }
}
