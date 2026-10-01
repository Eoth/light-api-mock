//! Keycloak client: password login (for the UI's login form) and local validation of access tokens.
//!
//! Tokens are only ever validated locally, against the realm's published keys (JWKS): signature with an asymmetric
//! algorithm, expiry, issuer, and the client the token was issued for. There is no fallback to Keycloak's
//! `userinfo` endpoint: that endpoint accepts a token issued to *any* client of the realm, so a fallback turns
//! every local refusal (another client's token, for one) into an acceptance, and it makes every invalid token cost
//! a call to Keycloak.
use crate::auth::AuthConfig;
use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, PublicKeyUse};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// The asymmetric algorithms Keycloak can sign access tokens with. HMAC and `none` are refused, so a token can
/// never be checked against a key that its sender could also hold.
const ACCEPTED_ALGORITHMS: &[Algorithm] = &[
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
    Algorithm::EdDSA,
];
const JWKS_TTL: Duration = Duration::from_secs(300);
/// Minimum delay between two key-set fetches triggered by an unknown key id (what a key rotation looks like), and
/// between two fetches of a set found empty, so that tokens carrying made-up key ids cannot turn each request into
/// a call to Keycloak.
const JWKS_REFETCH_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct KeycloakClient {
    http: reqwest::Client,
    config: AuthConfig,
    jwks: Arc<RwLock<CachedJwks>>,
}

#[derive(Default)]
struct CachedJwks {
    keys: Vec<Jwk>,
    fetched_at: Option<Instant>,
    refetched_for_unknown_key_at: Option<Instant>,
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
    preferred_username: String,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    aud: Option<Audience>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

impl KeycloakClaims {
    /// Keycloak names the client that requested a token in `azp` and lists in `aud` the clients it is meant for
    /// (often only `account`, unless an audience mapper is configured): either one naming this client will do.
    fn is_for(&self, client_id: &str) -> bool {
        let in_audience = match &self.aud {
            Some(Audience::One(aud)) => aud == client_id,
            Some(Audience::Many(auds)) => auds.iter().any(|a| a == client_id),
            None => false,
        };
        in_audience || self.azp.as_deref() == Some(client_id)
    }
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
            AuthError::InvalidCredentials => write!(f, "invalid credentials"),
            AuthError::TokenExpired => write!(f, "token expired"),
            AuthError::TokenInvalid(msg) => write!(f, "invalid token: {msg}"),
            AuthError::KeycloakUnavailable(msg) => write!(f, "Keycloak unavailable: {msg}"),
        }
    }
}

impl KeycloakClient {
    pub fn new(config: AuthConfig) -> Self {
        // Bounded so that a slow or unreachable Keycloak fails the request instead of holding it forever.
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            http,
            config,
            jwks: Arc::new(RwLock::new(CachedJwks::default())),
        }
    }

    fn realm_url(&self) -> String {
        format!(
            "{}/realms/{}",
            self.config.keycloak_url.trim_end_matches('/'),
            self.config.realm
        )
    }

    fn token_url(&self) -> String {
        format!("{}/protocol/openid-connect/token", self.realm_url())
    }

    fn certs_url(&self) -> String {
        format!("{}/protocol/openid-connect/certs", self.realm_url())
    }

    /// `KEYCLOAK_ISSUER` when Keycloak issues tokens under another URL than the one Mimicway reaches it through
    /// (a public hostname in front of an in-cluster service, for instance); the realm URL otherwise.
    fn issuer(&self) -> String {
        if self.config.issuer.is_empty() {
            self.realm_url()
        } else {
            self.config.issuer.clone()
        }
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
            .post(self.token_url())
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
            return Err(AuthError::KeycloakUnavailable(format!("{status}: {body}")));
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
            .post(self.token_url())
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

    /// The `preferred_username` of a valid access token issued by the realm to this client.
    pub async fn validate_token(&self, token: &str) -> Result<String, AuthError> {
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| AuthError::TokenInvalid(format!("malformed: {e}")))?;
        if !ACCEPTED_ALGORITHMS.contains(&header.alg) {
            return Err(AuthError::TokenInvalid(format!(
                "algorithm {:?} not accepted",
                header.alg
            )));
        }
        let key = self
            .verification_key(header.kid.as_deref(), header.alg)
            .await?;

        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[self.issuer()]);
        // Checked by `is_for` below, which also accepts `azp`.
        validation.validate_aud = false;
        let data =
            decode::<KeycloakClaims>(token, &key, &validation).map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::TokenInvalid(e.to_string()),
            })?;
        if !data.claims.is_for(&self.config.client_id) {
            return Err(AuthError::TokenInvalid(
                "issued for another client of the realm".into(),
            ));
        }
        Ok(data.claims.preferred_username)
    }

    async fn verification_key(
        &self,
        kid: Option<&str>,
        alg: Algorithm,
    ) -> Result<DecodingKey, AuthError> {
        self.refresh_jwks(false).await?;
        if let Some(key) = self.find_key(kid, alg).await {
            return key;
        }
        if self.refresh_jwks(true).await?
            && let Some(key) = self.find_key(kid, alg).await
        {
            return key;
        }
        Err(AuthError::TokenInvalid(
            "no published key matches it".into(),
        ))
    }

    async fn find_key(
        &self,
        kid: Option<&str>,
        alg: Algorithm,
    ) -> Option<Result<DecodingKey, AuthError>> {
        let jwks = self.jwks.read().await;
        let jwk = match kid {
            Some(kid) => jwks
                .keys
                .iter()
                .find(|k| k.common.key_id.as_deref() == Some(kid))?,
            None => jwks.keys.iter().find(|k| key_fits(k, alg))?,
        };
        if !key_fits(jwk, alg) {
            return Some(Err(AuthError::TokenInvalid(
                "its key does not match its algorithm".into(),
            )));
        }
        Some(DecodingKey::from_jwk(jwk).map_err(|e| AuthError::TokenInvalid(e.to_string())))
    }

    /// Fetches the realm's key set when the cached one is too old, or, when `unknown_key` is set, unless such a
    /// fetch already happened within the cooldown. Returns whether a fetch happened.
    async fn refresh_jwks(&self, unknown_key: bool) -> Result<bool, AuthError> {
        {
            let cached = self.jwks.read().await;
            let recent = |at: Option<Instant>, max_age| at.is_some_and(|t| t.elapsed() < max_age);
            let up_to_date = if unknown_key {
                recent(cached.refetched_for_unknown_key_at, JWKS_REFETCH_COOLDOWN)
            } else if cached.keys.is_empty() {
                recent(cached.fetched_at, JWKS_REFETCH_COOLDOWN)
            } else {
                recent(cached.fetched_at, JWKS_TTL)
            };
            if up_to_date {
                return Ok(false);
            }
        }
        let keys = self.fetch_jwks().await?;
        let mut cached = self.jwks.write().await;
        cached.keys = keys;
        cached.fetched_at = Some(Instant::now());
        if unknown_key {
            cached.refetched_for_unknown_key_at = cached.fetched_at;
        }
        Ok(true)
    }

    async fn fetch_jwks(&self) -> Result<Vec<Jwk>, AuthError> {
        let res = self
            .http
            .get(self.certs_url())
            .send()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;
        if !res.status().is_success() {
            return Err(AuthError::KeycloakUnavailable(format!(
                "key set fetch failed: {}",
                res.status()
            )));
        }
        let set: serde_json::Value = res
            .json()
            .await
            .map_err(|e| AuthError::KeycloakUnavailable(e.to_string()))?;
        // One key per entry, skipping the ones this library cannot read (a new key type, an unknown algorithm):
        // a single unreadable key must not make every token of the realm unverifiable.
        let keys = set
            .get("keys")
            .and_then(|k| k.as_array())
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| match serde_json::from_value::<Jwk>(k.clone()) {
                        Ok(jwk) => Some(jwk),
                        Err(e) => {
                            tracing::debug!(error = %e, "skipping an unreadable key of the realm key set");
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(keys)
    }
}

/// A signing key of the right family for `alg`; encryption keys and symmetric keys never qualify.
fn key_fits(jwk: &Jwk, alg: Algorithm) -> bool {
    use Algorithm::*;
    let family = matches!(
        (&jwk.algorithm, alg),
        (
            AlgorithmParameters::RSA(_),
            RS256 | RS384 | RS512 | PS256 | PS384 | PS512
        ) | (AlgorithmParameters::EllipticCurve(_), ES256 | ES384)
            | (AlgorithmParameters::OctetKeyPair(_), EdDSA)
    );
    family && jwk.common.public_key_use != Some(PublicKeyUse::Encryption)
}

#[cfg(test)]
mod tests;
