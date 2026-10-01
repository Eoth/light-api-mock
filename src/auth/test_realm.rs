//! A fake Keycloak realm for tests: it publishes real signing keys (EC P-256, generated per test) on the realm's
//! `certs` endpoint and signs real access tokens with them. Its `userinfo` endpoint accepts any bearer token, as
//! Keycloak does for a token issued to any client of the realm, so that a validation falling back to it would be
//! caught by the tests.
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

pub(crate) const REALM: &str = "test-realm";
pub(crate) const CLIENT_ID: &str = "lightmock";

pub(crate) struct SigningKey {
    kid: String,
    key: EncodingKey,
    pub(crate) jwk: serde_json::Value,
}

impl SigningKey {
    pub(crate) fn generate(kid: &str) -> Self {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        // Uncompressed point: 0x04 || x || y.
        let point = pair.public_key().as_ref();
        let jwk = serde_json::json!({
            "kty": "EC", "crv": "P-256", "use": "sig", "alg": "ES256", "kid": kid,
            "x": URL_SAFE_NO_PAD.encode(&point[1..33]),
            "y": URL_SAFE_NO_PAD.encode(&point[33..65]),
        });
        Self {
            kid: kid.to_string(),
            key: EncodingKey::from_ec_der(pkcs8.as_ref()),
            jwk,
        }
    }

    pub(crate) fn sign(&self, claims: &serde_json::Value) -> String {
        let header = Header {
            kid: Some(self.kid.clone()),
            ..Header::new(Algorithm::ES256)
        };
        jsonwebtoken::encode(&header, claims, &self.key).unwrap()
    }
}

pub(crate) struct FakeRealm {
    /// Keycloak's base URL (what `KEYCLOAK_URL` would hold).
    pub(crate) url: String,
    pub(crate) key: SigningKey,
    published: Arc<RwLock<Vec<serde_json::Value>>>,
    pub(crate) certs_fetches: Arc<AtomicUsize>,
}

impl FakeRealm {
    pub(crate) async fn start() -> Self {
        let key = SigningKey::generate("k1");
        let published = Arc::new(RwLock::new(vec![key.jwk.clone()]));
        let certs_fetches = Arc::new(AtomicUsize::new(0));
        let (keys, fetches) = (published.clone(), certs_fetches.clone());
        let app = axum::Router::new()
            .route(
                &format!("/realms/{REALM}/protocol/openid-connect/certs"),
                axum::routing::get(move || {
                    fetches.fetch_add(1, Ordering::SeqCst);
                    let keys = keys.read().unwrap().clone();
                    async move { axum::Json(serde_json::json!({ "keys": keys })) }
                }),
            )
            .route(
                &format!("/realms/{REALM}/protocol/openid-connect/userinfo"),
                axum::routing::get(|| async {
                    axum::Json(serde_json::json!({ "preferred_username": "anyone" }))
                }),
            );
        let url = crate::server::test_support::serve(app).await;
        Self {
            url,
            key,
            published,
            certs_fetches,
        }
    }

    pub(crate) fn issuer(&self) -> String {
        format!("{}/realms/{REALM}", self.url)
    }

    /// Claims of a valid token issued by this realm to lightMock's client, as Keycloak shapes them by default.
    pub(crate) fn claims(&self, username: &str) -> serde_json::Value {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        serde_json::json!({
            "iss": self.issuer(), "azp": CLIENT_ID, "aud": "account", "exp": now + 300, "iat": now,
            "preferred_username": username,
        })
    }

    pub(crate) fn token_for(&self, username: &str) -> String {
        self.key.sign(&self.claims(username))
    }

    pub(crate) fn publish(&self, jwk: serde_json::Value) {
        self.published.write().unwrap().push(jwk);
    }

    pub(crate) fn auth_config(&self, super_admins: Vec<String>) -> crate::auth::AuthConfig {
        crate::auth::AuthConfig {
            enabled: true,
            keycloak_url: self.url.clone(),
            realm: REALM.into(),
            client_id: CLIENT_ID.into(),
            issuer: String::new(),
            super_admins,
            show_reset_button: false,
        }
    }
}
