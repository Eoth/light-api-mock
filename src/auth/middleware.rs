use crate::auth::AuthConfig;
use crate::auth::keycloak::KeycloakClient;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub username: String,
    pub is_super_admin: bool,
}

impl AuthUser {
    pub fn anonymous() -> Self {
        Self {
            username: "anonymous".into(),
            is_super_admin: true,
        }
    }
}

pub async fn auth_middleware(
    auth_config: AuthConfig,
    keycloak: Option<KeycloakClient>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    let no_auth_paths = ["/api/health", "/api/auth/status", "/api/auth/login", "/api/auth/validate"];
    // Assets statiques de la SPA (index.html, bundle JS/CSS, favicon) : voir
    // `is_static_asset_route` (src/server/validation.rs) pour la justification
    // complete. Sans ce bypass, un navigateur sans token ne peut meme pas
    // charger le JS qui affiche l'ecran de connexion (LoginForm.svelte) quand
    // AUTH_ENABLED=true. Volontairement distinct de `no_auth_paths` ci-dessus
    // (egalite stricte) : ce bypass ne doit JAMAIS etre etendu a un prefixe
    // "/api" — voir les tests de non-regression dans validation.rs.
    if no_auth_paths.iter().any(|p| path == *p) || crate::server::validation::is_static_asset_route(&path) {
        req.extensions_mut().insert(AuthUser::anonymous());
        return next.run(req).await;
    }

    if !auth_config.enabled {
        req.extensions_mut().insert(AuthUser::anonymous());
        return next.run(req).await;
    }

    let kc = match keycloak.as_ref() {
        Some(kc) => kc,
        None => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from);

    let Some(token) = token else {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "Token manquant"})),
        )
            .into_response();
    };

    match kc.validate_token(&token).await {
        Ok(username) => {
            let is_super_admin = auth_config.is_super_admin(&username);
            req.extensions_mut().insert(AuthUser {
                username,
                is_super_admin,
            });
            next.run(req).await
        }
        Err(e) => {
            tracing::debug!(error = %e, "token validation failed");
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({"error": format!("{e}")})),
            )
                .into_response()
        }
    }
}

pub fn extract_user(extensions: &axum::http::Extensions) -> AuthUser {
    extensions
        .get::<AuthUser>()
        .cloned()
        .unwrap_or_else(AuthUser::anonymous)
}

// Point de securite le plus sensible du projet (audit de modularite,
// docs/audit-modularite.md) : auparavant couvert uniquement de facon
// indirecte par frontend/e2e/security.spec.js, sans qu'aucune branche de
// auth_middleware ne soit isolee par un test Rust. Reprend le pattern deja
// etabli dans server::intercept::tests / server::api::tests (vrai routeur
// Axum via server::build_router + vrai listener TCP + reqwest::Client),
// plutot qu'un style tower::oneshot inexistant dans ce projet.
//
// Pour le cas "token valide", un faux serveur Keycloak minimal
// (spawn_fake_keycloak) sert uniquement l'endpoint userinfo : l'endpoint
// certs n'est volontairement pas enregistre (404 naturel d'axum), ce qui
// fait echouer validate_jwt_local() des le fetch JWKS et declenche
// systematiquement le repli vers validate_via_userinfo() — suffisant pour
// exercer reellement le chemin "token accepte par Keycloak" sans avoir a
// signer un vrai JWT RS256 dans les tests.
#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::HeaderMap;

    fn enabled_auth_config(keycloak_url: String, super_admins: Vec<String>) -> AuthConfig {
        AuthConfig {
            enabled: true,
            keycloak_url,
            realm: "test-realm".into(),
            client_id: "lightmock".into(),
            super_admins,
            show_reset_button: false,
        }
    }

    fn disabled_auth_config() -> AuthConfig {
        AuthConfig {
            enabled: false,
            keycloak_url: String::new(),
            realm: String::new(),
            client_id: String::new(),
            super_admins: vec![],
            show_reset_button: false,
        }
    }

    // Reprend le pattern spawn_test_app deja etabli dans server::api::tests /
    // server::intercept::tests : vrai serveur Axum (server::build_router) sur
    // un vrai port TCP, appele ensuite via reqwest::Client. Permet de tester
    // auth_middleware exactement comme il tourne en production (layer Axum
    // reel), pas une version isolee reimplementee pour le test.
    async fn spawn_test_app(auth_config: AuthConfig, keycloak: Option<KeycloakClient>) -> String {
        let data_dir = std::env::temp_dir().join(format!(
            "lightmock-auth-mw-test-{}",
            fastrand::u64(..)
        ));
        std::fs::create_dir_all(&data_dir).unwrap();
        let store = crate::store::MockStore::new(data_dir.join("mock-config.yaml"));
        store
            .replace(crate::models::MockConfig {
                services: vec![],
                groups: vec![],
            })
            .await
            .unwrap();
        store.flush().await;

        #[cfg(feature = "messaging-kafka")]
        let messaging = crate::messaging::MessagingState {
            message_log: crate::messaging::message_log::MessageLog::new(),
            reply_topic: None,
            publisher: crate::messaging::consumer::Publisher::None,
        };
        let state = crate::server::AppState {
            store,
            proxy: crate::engine::ProxyClient::new(),
            seq_counters: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            request_log: crate::server::request_log::RequestLog::new(),
            auth_config,
            keycloak,
            script_engine: crate::engine::script::ScriptEngine::new(),
            ping_cache: crate::server::ping::PingCache::new(),
            #[cfg(feature = "messaging-kafka")]
            messaging,
        };
        let app = crate::server::build_router(state, &data_dir);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://127.0.0.1:{port}/api")
    }

    #[derive(Clone)]
    struct FakeKcState {
        valid_token: String,
        valid_username: String,
    }

    async fn fake_userinfo(State(fake): State<FakeKcState>, headers: HeaderMap) -> Response {
        let expected = format!("Bearer {}", fake.valid_token);
        let actual = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if actual == expected {
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({ "preferred_username": fake.valid_username })),
            )
                .into_response()
        } else {
            StatusCode::UNAUTHORIZED.into_response()
        }
    }

    // Ne sert QUE /realms/test-realm/.../userinfo (pas de route /certs : 404
    // naturel, cf commentaire au-dessus du mod tests) — suffisant pour
    // exercer validate_token() via son repli userinfo.
    async fn spawn_fake_keycloak(valid_token: &str, valid_username: &str) -> String {
        let fake_state = FakeKcState {
            valid_token: valid_token.to_string(),
            valid_username: valid_username.to_string(),
        };
        let app = axum::Router::new()
            .route(
                "/realms/test-realm/protocol/openid-connect/userinfo",
                axum::routing::get(fake_userinfo),
            )
            .with_state(fake_state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://127.0.0.1:{port}")
    }

    #[tokio::test]
    async fn missing_token_rejected_on_protected_route() {
        // Branche: aucun header Authorization sur une route protegee (hors
        // liste de bypass) -> rejet 401 "Token manquant", sans meme
        // contacter Keycloak.
        let kc_url = spawn_fake_keycloak("good-token", "alice").await;
        let auth_config = enabled_auth_config(kc_url, vec![]);
        let keycloak = Some(KeycloakClient::new(auth_config.clone()));
        let base = spawn_test_app(auth_config, keycloak).await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/services"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 401);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"], "Token manquant");
    }

    #[tokio::test]
    async fn non_bearer_scheme_treated_as_missing_token() {
        // Branche: header Authorization present mais sans le prefixe
        // "Bearer " (ex. schema Basic) -> strip_prefix() echoue, traite
        // exactement comme une absence de token -> 401.
        let kc_url = spawn_fake_keycloak("good-token", "alice").await;
        let auth_config = enabled_auth_config(kc_url, vec![]);
        let keycloak = Some(KeycloakClient::new(auth_config.clone()));
        let base = spawn_test_app(auth_config, keycloak).await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/services"))
            .header("authorization", "Basic dXNlcjpwYXNz")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 401);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["error"], "Token manquant");
    }

    #[tokio::test]
    async fn invalid_token_rejected() {
        // Branche: token present mais invalide/rejete par Keycloak (ici via
        // le repli userinfo, qui renvoie 401 pour un token qui ne
        // correspond pas) -> 401.
        let kc_url = spawn_fake_keycloak("good-token", "alice").await;
        let auth_config = enabled_auth_config(kc_url, vec![]);
        let keycloak = Some(KeycloakClient::new(auth_config.clone()));
        let base = spawn_test_app(auth_config, keycloak).await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/services"))
            .header("authorization", "Bearer wrong-token")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[tokio::test]
    async fn valid_token_injects_authuser_readable_by_next_handler() {
        // Branche: token valide -> AuthUser correctement injecte dans les
        // extensions de la requete ET lisible par le handler suivant.
        // GET /api/auth/me (Extension<AuthUser>, hors liste de bypass) sert
        // de sonde directe pour verifier le contenu exact de l'AuthUser
        // injecte par le middleware (username + is_super_admin).
        let kc_url = spawn_fake_keycloak("good-token", "alice").await;
        let auth_config = enabled_auth_config(kc_url, vec!["alice".into()]);
        let keycloak = Some(KeycloakClient::new(auth_config.clone()));
        let base = spawn_test_app(auth_config, keycloak).await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/auth/me"))
            .header("authorization", "Bearer good-token")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["username"], "alice");
        assert_eq!(body["is_super_admin"], true);
    }

    #[tokio::test]
    async fn bypass_routes_accessible_without_any_token() {
        // Branche: chaque route de no_auth_paths reste accessible sans
        // aucun token, meme avec l'auth activee ET Keycloak volontairement
        // absent (None) — preuve que le bypass est teste AVANT toute
        // dependance a Keycloak (sinon ces 4 routes renverraient 500, pas
        // leur reponse normale).
        let auth_config = enabled_auth_config("http://127.0.0.1:1".into(), vec![]);
        let base = spawn_test_app(auth_config, None).await;
        let client = reqwest::Client::new();

        let health = client.get(format!("{base}/health")).send().await.unwrap();
        assert_eq!(health.status().as_u16(), 200, "/api/health doit etre exempt d'auth");

        let status = client
            .get(format!("{base}/auth/status"))
            .send()
            .await
            .unwrap();
        assert_eq!(status.status().as_u16(), 200, "/api/auth/status doit etre exempt d'auth");

        // /auth/login et /auth/validate atteignent bien leur handler (qui
        // echoue ensuite pour une tout autre raison : Keycloak non
        // configure -> 400 Validation) — la preuve recherchee est l'absence
        // du 401 "Token manquant" qu'un blocage par le middleware aurait
        // produit.
        let login = client
            .post(format!("{base}/auth/login"))
            .json(&serde_json::json!({"username": "u", "password": "p"}))
            .send()
            .await
            .unwrap();
        assert_eq!(login.status().as_u16(), 400, "/api/auth/login doit etre exempt d'auth (echoue ensuite sur Keycloak non configure, pas sur le token)");

        let validate = client
            .post(format!("{base}/auth/validate"))
            .json(&serde_json::json!({"token": "whatever"}))
            .send()
            .await
            .unwrap();
        assert_eq!(validate.status().as_u16(), 400, "/api/auth/validate doit etre exempt d'auth (meme raisonnement que /auth/login)");
    }

    #[tokio::test]
    async fn static_asset_routes_accessible_without_any_token() {
        // Branche: la coquille de la SPA (racine, index.html, bundle
        // assets/, favicon) reste chargeable sans token meme avec l'auth
        // activee ET Keycloak volontairement absent (None) — sinon un
        // navigateur ne recevrait jamais le JS qui affiche LoginForm.svelte.
        // spawn_test_app sert `data_dir` comme static_dir (aucun fichier
        // reel dedans) : on verifie donc l'ABSENCE du 401 "Token manquant"
        // (preuve que le bypass a bien agi), pas un contenu de fichier reel
        // — ServeDir renverra 404 pour un fichier absent, ce qui est attendu
        // ici et ne remet pas en cause le test.
        let auth_config = enabled_auth_config("http://127.0.0.1:1".into(), vec![]);
        let base = spawn_test_app(auth_config, None).await;
        let root = base.trim_end_matches("/api");
        let client = reqwest::Client::new();

        for path in ["/", "/index.html", "/assets/main.js", "/favicon.ico"] {
            let resp = client.get(format!("{root}{path}")).send().await.unwrap();
            assert_ne!(
                resp.status().as_u16(),
                401,
                "asset statique {path} ne doit jamais exiger de token"
            );
        }
    }

    #[tokio::test]
    async fn static_asset_bypass_does_not_widen_api_protection() {
        // Regression cible : le bypass des assets statiques ne doit jamais
        // affaiblir la protection d'une route /api/* arbitraire (hors les 4
        // routes deja exemptees). Prouve au niveau du vrai routeur Axum, pas
        // seulement au niveau unitaire de is_static_asset_route
        // (validation.rs), que le comportement bout-en-bout reste correct.
        let kc_url = spawn_fake_keycloak("good-token", "alice").await;
        let auth_config = enabled_auth_config(kc_url, vec![]);
        let keycloak = Some(KeycloakClient::new(auth_config.clone()));
        let base = spawn_test_app(auth_config, keycloak).await;
        let client = reqwest::Client::new();

        let resp = client
            .get(format!("{base}/services"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status().as_u16(),
            401,
            "/api/services doit rester protege malgre le bypass des assets statiques"
        );
    }

    #[tokio::test]
    async fn auth_disabled_injects_anonymous_super_admin() {
        // Branche: AUTH_ENABLED=false -> AuthUser::anonymous()
        // (is_super_admin=true) injecte pour TOUTE route, y compris une
        // route qui exigerait normalement un token quand l'auth est active
        // (/api/auth/me n'est pas dans la liste de bypass).
        let base = spawn_test_app(disabled_auth_config(), None).await;
        let client = reqwest::Client::new();
        let resp = client.get(format!("{base}/auth/me")).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["username"], "anonymous");
        assert_eq!(body["is_super_admin"], true);
    }

    #[tokio::test]
    async fn misconfigured_auth_without_keycloak_fails_closed() {
        // Branche defensive: AUTH_ENABLED=true mais aucun KeycloakClient
        // construit cote etat serveur (config incoherente) -> 500 explicite
        // plutot qu'un acces silencieusement autorise. Cette garde
        // s'execute AVANT l'extraction du token : aucun header Authorization
        // n'est meme fourni ici.
        let auth_config = enabled_auth_config("http://127.0.0.1:1".into(), vec![]);
        let base = spawn_test_app(auth_config, None).await;
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{base}/services"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 500);
    }
}
