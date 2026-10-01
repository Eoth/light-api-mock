//! Credentials seen in traffic never leave the request they came with. The request log, the traffic observation
//! and the rule suggestions keep the header *names* (an `Exists` condition can still be tested against a capture)
//! but replace the values of credential headers, and URLs lose their user-info part before being logged. Without
//! this, `GET /api/logs` handed out the bearer tokens and session cookies of the applications under test, and a
//! suggested rule copied the real backend's `Set-Cookie` into the saved configuration.
use std::collections::HashMap;
use std::sync::LazyLock;

pub const REDACTED: &str = "[redacted]";

const ALWAYS_REDACTED: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "x-amz-security-token",
];

/// Extra header names to redact, from `REDACT_HEADERS` (comma-separated, case-insensitive), read once.
static EXTRA_REDACTED: LazyLock<Vec<String>> =
    LazyLock::new(|| parse_header_list(&std::env::var("REDACT_HEADERS").unwrap_or_default()));

fn parse_header_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect()
}

pub fn is_sensitive_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ALWAYS_REDACTED.contains(&name.as_str()) || EXTRA_REDACTED.contains(&name)
}

pub fn redact_headers(headers: &HashMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            let value = if is_sensitive_header(name) {
                REDACTED.to_string()
            } else {
                value.clone()
            };
            (name.clone(), value)
        })
        .collect()
}

/// `url` without its `user:password@` part, for logs; returned unchanged when it has none or does not parse.
pub fn redact_url_credentials(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) if !parsed.username().is_empty() || parsed.password().is_some() => {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.to_string()
        }
        _ => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_headers_are_redacted_whatever_their_case() {
        let headers: HashMap<String, String> = [
            ("Authorization", "Bearer secret"),
            ("cookie", "session=secret"),
            ("X-Api-Key", "secret"),
            ("content-type", "application/json"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let redacted = redact_headers(&headers);
        assert_eq!(redacted["Authorization"], REDACTED);
        assert_eq!(redacted["cookie"], REDACTED);
        assert_eq!(redacted["X-Api-Key"], REDACTED);
        assert_eq!(redacted["content-type"], "application/json");
    }

    #[test]
    fn extra_header_names_are_parsed_case_insensitively() {
        assert_eq!(
            parse_header_list(" X-Tenant-Secret, ,x-other "),
            vec!["x-tenant-secret", "x-other"]
        );
    }

    #[test]
    fn url_credentials_are_removed() {
        assert_eq!(
            redact_url_credentials("https://user:pass@api.example.com/v1?q=1"),
            "https://api.example.com/v1?q=1"
        );
        assert_eq!(
            redact_url_credentials("http://api.example.com/v1"),
            "http://api.example.com/v1"
        );
        assert_eq!(redact_url_credentials("not a url"), "not a url");
    }

    #[tokio::test]
    async fn the_request_log_api_never_returns_credentials() {
        use crate::server::test_support::{
            auth_disabled, mock_service, serve, temp_data_dir, test_state,
        };
        let data_dir = temp_data_dir("redaction");
        let config = crate::models::MockConfig {
            services: vec![mock_service("orders", "ok")],
            groups: vec![],
        };
        let state = test_state(&data_dir, config, auth_disabled()).await;
        let root = serve(crate::server::build_router(state, &data_dir)).await;
        let client = reqwest::Client::new();
        client
            .get(format!("{root}/orders/1"))
            .header("authorization", "Bearer top-secret")
            .header("cookie", "session=top-secret")
            .header("x-trace", "kept")
            .send()
            .await
            .unwrap();

        let logs = client
            .get(format!("{root}/api/logs"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(!logs.contains("top-secret"), "{logs}");
        let logs: serde_json::Value = serde_json::from_str(&logs).unwrap();
        let headers = &logs[0]["captured"]["headers"];
        assert_eq!(headers["authorization"], REDACTED);
        assert_eq!(headers["x-trace"], "kept");
    }

    #[test]
    fn observed_exchanges_keep_no_credentials() {
        let headers = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let exchange = crate::server::observation::ObservedExchange::new(
            HashMap::new(),
            headers(&[("authorization", "Bearer top-secret")]),
            b"",
            None,
            200,
            headers(&[("set-cookie", "session=top-secret"), ("x-env", "prod")]),
            b"ok",
            None,
        );
        assert_eq!(exchange.request_headers["authorization"], REDACTED);
        assert_eq!(exchange.response_headers["set-cookie"], REDACTED);
        assert_eq!(exchange.response_headers["x-env"], "prod");
    }
}
