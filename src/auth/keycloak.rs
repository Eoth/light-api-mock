use crate::auth::AuthConfig;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct KeycloakClient {
    http: reqwest::Client,
    config: AuthConfig,
    jwks: Arc<RwLock<CachedJwks>>,
}

struct CachedJwks {
    keys: Vec<JwkEntry>,
    fetched_at: std::time::Instant,
}

#[derive(Debug, Clone, Deserialize)]
struct JwkSet {
    keys: Vec<JwkEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct JwkEntry {
    #[serde(default)]
    kid: Option<String>,
    kty: String,
    #[serde(default)]
    #[allow(dead_code)]
    alg: Option<String>,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
    #[serde(rename = "use", default)]
    use_: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct KeycloakClaims {
    #[allow(dead_code)]
    exp: usize,
    #[allow(dead_code)]
    iss: String,
    preferred_username: String,
    #[allow(dead_code)]
    azp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    preferred_username: String,
}

#[derive(Debug)]
pub enum AuthError {
    InvalidCredentials,
    TokenExpired,
    TokenInvalid(String),
    KeycloakUnavailable(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "Identifiants invalides"),
            AuthError::TokenExpired => write!(f, "Token expire"),
            AuthError::TokenInvalid(msg) => write!(f, "Token invalide: {msg}"),
            AuthError::KeycloakUnavailable(msg) => write!(f, "Keycloak indisponible: {msg}"),
        }
    }
}

const JWKS_TTL_SECS: u64 = 300;

impl KeycloakClient {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
            jwks: Arc::new(RwLock::new(CachedJwks {
                keys: vec![],
                fetched_at: std::time::Instant::now()
                    - std::time::Duration::from_secs(JWKS_TTL_SECS + 1),
            })),
        }
    }

    fn token_url(&self) -> String {
        format!(
            "{}/realms/{}/protocol/openid-connect/token",
            self.config.keycloak_url.trim_end_matches('/'),
            self.config.realm
        )
    }

    fn certs_url(&self) -> String {
        format!(
            "{}/realms/{}/protocol/openid-connect/certs",
            self.config.keycloak_url.trim_end_matches('/'),
            self.config.realm
        )
    }

    fn userinfo_url(&self) -> String {
        format!(
            "{}/realms/{}/protocol/openid-connect/userinfo",
            self.config.keycloak_url.trim_end_matches('/'),
            self.config.realm
        )
    }

    fn issuer(&self) -> String {
        format!(
            "{}/realms/{}",
            self.config.keycloak_url.trim_end_matches('/'),
            self.config.realm
        )
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<TokenResponse, AuthError> {
        let params = [
            ("grant_type", "password"),
            ("client_id", &self.config.client_id),
            ("username", username),
            ("password", password),
        ];

        let res = self
            .http
            .post(&self.token_url())
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;

        if res.status() == reqwest::StatusCode::UNAUTHORIZED
            || res.status() == reqwest::StatusCode::BAD_REQUEST
        {
            return Err(AuthError::InvalidCredentials);
        }

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(AuthError::KeycloakUnavailable(format!(
                "{status}: {body}"
            )));
        }

        res.json::<TokenResponse>()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, AuthError> {
        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", &self.config.client_id),
            ("refresh_token", refresh_token),
        ];

        let res = self
            .http
            .post(&self.token_url())
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AuthError::TokenExpired);
        }

        res.json::<TokenResponse>()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))
    }

    pub async fn validate_token(&self, token: &str) -> Result<String, AuthError> {
        match self.validate_jwt_local(token).await {
            Ok(username) => Ok(username),
            Err(_) => self.validate_via_userinfo(token).await,
        }
    }

    async fn validate_jwt_local(&self, token: &str) -> Result<String, AuthError> {
        self.refresh_jwks_if_stale().await?;

        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| AuthError::TokenInvalid(e.to_string()))?;

        let jwks = self.jwks.read().await;

        let key_entry = header
            .kid
            .as_ref()
            .and_then(|kid| jwks.keys.iter().find(|k| k.kid.as_deref() == Some(kid)))
            .or_else(|| {
                jwks.keys
                    .iter()
                    .find(|k| k.use_.as_deref() == Some("sig") && k.kty == "RSA")
            })
            .ok_or_else(|| AuthError::TokenInvalid("No matching JWK found".into()))?;

        let n = key_entry
            .n
            .as_ref()
            .ok_or_else(|| AuthError::TokenInvalid("JWK missing 'n'".into()))?;
        let e = key_entry
            .e
            .as_ref()
            .ok_or_else(|| AuthError::TokenInvalid("JWK missing 'e'".into()))?;

        let decoding_key = DecodingKey::from_rsa_components(n, e)
            .map_err(|e| AuthError::TokenInvalid(e.to_string()))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer()]);
        validation.set_audience(&[&self.config.client_id]);

        let token_data = decode::<KeycloakClaims>(token, &decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::TokenInvalid(e.to_string()),
            })?;

        Ok(token_data.claims.preferred_username)
    }

    async fn validate_via_userinfo(&self, token: &str) -> Result<String, AuthError> {
        let res = self
            .http
            .get(&self.userinfo_url())
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AuthError::TokenInvalid("userinfo validation failed".into()));
        }

        let info: UserInfoResponse = res
            .json()
            .await
            .map_err(|e| AuthError::TokenInvalid(e.to_string()))?;

        Ok(info.preferred_username)
    }

    async fn refresh_jwks_if_stale(&self) -> Result<(), AuthError> {
        {
            let cached = self.jwks.read().await;
            if cached.fetched_at.elapsed().as_secs() < JWKS_TTL_SECS && !cached.keys.is_empty() {
                return Ok(());
            }
        }

        let res = self
            .http
            .get(&self.certs_url())
            .send()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AuthError::KeycloakUnavailable(format!(
                "JWKS fetch failed: {}",
                res.status()
            )));
        }

        let jwk_set: JwkSet = res
            .json()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;

        let mut cached = self.jwks.write().await;
        cached.keys = jwk_set.keys;
        cached.fetched_at = std::time::Instant::now();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AuthConfig {
        AuthConfig {
            enabled: true,
            keycloak_url: "https://keycloak.example.com".into(),
            realm: "entreprise".into(),
            client_id: "lightmock".into(),
            super_admins: vec!["admin".into()],
        }
    }

    #[test]
    fn token_url_construction() {
        let client = KeycloakClient::new(test_config());
        assert_eq!(
            client.token_url(),
            "https://keycloak.example.com/realms/entreprise/protocol/openid-connect/token"
        );
    }

    #[test]
    fn certs_url_construction() {
        let client = KeycloakClient::new(test_config());
        assert_eq!(
            client.certs_url(),
            "https://keycloak.example.com/realms/entreprise/protocol/openid-connect/certs"
        );
    }

    #[test]
    fn issuer_construction() {
        let client = KeycloakClient::new(test_config());
        assert_eq!(
            client.issuer(),
            "https://keycloak.example.com/realms/entreprise"
        );
    }

    #[test]
    fn trailing_slash_in_url_handled() {
        let cfg = AuthConfig {
            keycloak_url: "https://keycloak.example.com/".into(),
            ..test_config()
        };
        let client = KeycloakClient::new(cfg);
        assert_eq!(
            client.token_url(),
            "https://keycloak.example.com/realms/entreprise/protocol/openid-connect/token"
        );
    }

    #[test]
    fn parse_token_response() {
        let json = r#"{"access_token":"abc","refresh_token":"def","expires_in":300}"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "abc");
        assert_eq!(resp.refresh_token.as_deref(), Some("def"));
        assert_eq!(resp.expires_in, 300);
    }

    #[test]
    fn parse_token_response_minimal() {
        let json = r#"{"access_token":"abc"}"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "abc");
        assert!(resp.refresh_token.is_none());
        assert_eq!(resp.expires_in, 0);
    }
}
