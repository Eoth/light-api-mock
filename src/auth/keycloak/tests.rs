use super::*;
use crate::auth::test_realm::{FakeRealm, SigningKey};
use std::sync::atomic::Ordering;

fn test_config() -> AuthConfig {
    AuthConfig {
        enabled: true,
        keycloak_url: "https://keycloak.example.com".into(),
        realm: "entreprise".into(),
        client_id: "mimicway".into(),
        issuer: String::new(),
        super_admins: vec!["admin".into()],
        show_reset_button: false,
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
fn issuer_defaults_to_the_realm_url_and_can_be_overridden() {
    assert_eq!(
        KeycloakClient::new(test_config()).issuer(),
        "https://keycloak.example.com/realms/entreprise"
    );
    let cfg = AuthConfig {
        issuer: "https://sso.example.com/realms/entreprise".into(),
        ..test_config()
    };
    assert_eq!(
        KeycloakClient::new(cfg).issuer(),
        "https://sso.example.com/realms/entreprise"
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

async fn realm_and_client() -> (FakeRealm, KeycloakClient) {
    let realm = FakeRealm::start().await;
    let client = KeycloakClient::new(realm.auth_config(vec![]));
    (realm, client)
}

#[tokio::test]
async fn accepts_a_token_of_the_realm_issued_to_this_client() {
    let (realm, client) = realm_and_client().await;
    let username = client.validate_token(&realm.token_for("alice")).await;
    assert_eq!(username.unwrap(), "alice");
    // A token whose audience names the client is accepted too.
    let mut claims = realm.claims("bob");
    claims["azp"] = "other".into();
    claims["aud"] = serde_json::json!(["account", "mimicway"]);
    assert_eq!(
        client
            .validate_token(&realm.key.sign(&claims))
            .await
            .unwrap(),
        "bob"
    );
}

#[tokio::test]
async fn refuses_a_token_issued_to_another_client_of_the_realm() {
    // Keycloak's userinfo accepts such a token: validating through it let any application of the realm in.
    let (realm, client) = realm_and_client().await;
    let mut claims = realm.claims("mallory");
    claims["azp"] = "another-app".into();
    let result = client.validate_token(&realm.key.sign(&claims)).await;
    assert!(
        matches!(result, Err(AuthError::TokenInvalid(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn refuses_expired_tokens_and_foreign_issuers() {
    let (realm, client) = realm_and_client().await;
    let mut expired = realm.claims("alice");
    expired["exp"] = 1_000.into();
    assert!(matches!(
        client.validate_token(&realm.key.sign(&expired)).await,
        Err(AuthError::TokenExpired)
    ));
    let mut foreign = realm.claims("alice");
    foreign["iss"] = format!("{}/realms/other-realm", realm.url).into();
    assert!(matches!(
        client.validate_token(&realm.key.sign(&foreign)).await,
        Err(AuthError::TokenInvalid(_))
    ));
}

#[tokio::test]
async fn refuses_symmetric_and_unsigned_tokens() {
    let (realm, client) = realm_and_client().await;
    let claims = realm.claims("alice");
    let hmac = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"guessable"),
    )
    .unwrap();
    assert!(client.validate_token(&hmac).await.is_err());
    let payload = b64url(&serde_json::to_vec(&claims).unwrap());
    let unsigned = format!("eyJhbGciOiJub25lIn0.{payload}.");
    assert!(client.validate_token(&unsigned).await.is_err());
}

fn b64url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[tokio::test]
async fn refuses_a_token_signed_by_an_unpublished_key() {
    let (realm, client) = realm_and_client().await;
    let impostor = SigningKey::generate("k1");
    let result = client
        .validate_token(&impostor.sign(&realm.claims("alice")))
        .await;
    assert!(
        matches!(result, Err(AuthError::TokenInvalid(_))),
        "{result:?}"
    );
}

#[tokio::test]
async fn picks_up_a_rotated_key_but_refetches_at_most_once_per_cooldown() {
    let (realm, client) = realm_and_client().await;
    assert!(
        client
            .validate_token(&realm.token_for("alice"))
            .await
            .is_ok()
    );
    assert_eq!(realm.certs_fetches.load(Ordering::SeqCst), 1);

    let rotated = SigningKey::generate("k2");
    realm.publish(rotated.jwk.clone());
    // A key id never seen triggers one refetch; it was published since, so the token is accepted.
    // Within the cooldown, more unknown key ids do not reach Keycloak again.
    let first = client
        .validate_token(&rotated.sign(&realm.claims("alice")))
        .await;
    assert_eq!(first.unwrap(), "alice");
    for i in 0..5 {
        let forged = SigningKey::generate(&format!("made-up-{i}"));
        assert!(
            client
                .validate_token(&forged.sign(&realm.claims("alice")))
                .await
                .is_err()
        );
    }
    assert_eq!(realm.certs_fetches.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn skips_unreadable_keys_of_the_key_set() {
    let (realm, client) = realm_and_client().await;
    realm
        .publish(serde_json::json!({"kty": "AKP", "alg": "ML-DSA-65", "kid": "pq", "pub": "AAAA"}));
    realm.publish(serde_json::json!({"kty": "RSA", "alg": "RSA-OAEP", "use": "enc", "kid": "enc", "n": "AQAB", "e": "AQAB"}));
    assert_eq!(
        client
            .validate_token(&realm.token_for("alice"))
            .await
            .unwrap(),
        "alice"
    );
}

#[tokio::test]
async fn an_unreachable_keycloak_is_reported_as_unavailable() {
    let client = KeycloakClient::new(AuthConfig {
        keycloak_url: "http://127.0.0.1:1".into(),
        ..test_config()
    });
    let token = SigningKey::generate("k1").sign(&serde_json::json!({"preferred_username": "a"}));
    assert!(matches!(
        client.validate_token(&token).await,
        Err(AuthError::KeycloakUnavailable(_))
    ));
}
