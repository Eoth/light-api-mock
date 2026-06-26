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
    if no_auth_paths.iter().any(|p| path == *p) {
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
