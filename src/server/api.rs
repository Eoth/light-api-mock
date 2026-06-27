use crate::auth::middleware::AuthUser;
use crate::auth::{can_access_service, can_manage_group, visible_services};
use crate::models::{Group, MockConfig, Service};
use crate::server::AppState;
use std::sync::Arc;
use crate::server::request_log::LogEntry;
use crate::server::validation::validate_service;
use axum::Extension;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete as delete_route, get, post, put};
use axum::{Json, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/auth/status", get(auth_status))
        .route("/auth/login", post(login))
        .route("/auth/validate", post(validate_token))
        .route("/auth/me", get(get_me))
        .route("/config", get(get_config).put(put_config))
        .route("/config/reset", delete_route(reset_config))
        .route("/services", get(list_services).post(create_service))
        .route(
            "/services/:name",
            get(get_service).put(update_service).delete(delete_service),
        )
        .route("/services/:name/toggle", put(toggle_service))
        .route("/services/:name/rules/reorder", put(reorder_rules))
        .route("/logs", get(get_logs))
        .route("/groups", get(list_groups).post(create_group))
        .route(
            "/groups/:name",
            get(get_group).put(update_group).delete(delete_group),
        )
        .route("/groups/:name/members", put(update_group_members))
}

// --------------- Health ---------------

async fn health() -> StatusCode {
    StatusCode::OK
}

// --------------- Auth ---------------

#[derive(serde::Serialize)]
struct AuthStatusResponse {
    enabled: bool,
}

async fn auth_status(State(state): State<AppState>) -> Json<AuthStatusResponse> {
    Json(AuthStatusResponse {
        enabled: state.auth_config.enabled,
    })
}

#[derive(serde::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(serde::Serialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    username: String,
    is_super_admin: bool,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    if !state.auth_config.enabled {
        return Err(AppError::Validation(
            "L'authentification n'est pas activee".into(),
        ));
    }

    let kc = state
        .keycloak
        .as_ref()
        .ok_or(AppError::Validation("Auth non configuree".into()))?;

    let tokens = kc.login(&req.username, &req.password).await.map_err(|e| {
        use crate::auth::keycloak::AuthError;
        match e {
            AuthError::InvalidCredentials => AppError::Unauthorized,
            _ => AppError::Validation(format!("{e}")),
        }
    })?;

    let is_super_admin = state.auth_config.is_super_admin(&req.username);

    Ok(Json(LoginResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in,
        username: req.username,
        is_super_admin,
    }))
}

#[derive(serde::Deserialize)]
struct ValidateRequest {
    token: String,
}

async fn validate_token(
    State(state): State<AppState>,
    Json(req): Json<ValidateRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    if !state.auth_config.enabled {
        return Err(AppError::Validation(
            "L'authentification n'est pas activee".into(),
        ));
    }

    let kc = state
        .keycloak
        .as_ref()
        .ok_or(AppError::Validation("Auth non configuree".into()))?;

    let username = kc
        .validate_token(&req.token)
        .await
        .map_err(|_| AppError::Unauthorized)?;

    let is_super_admin = state.auth_config.is_super_admin(&username);

    Ok(Json(LoginResponse {
        access_token: req.token,
        refresh_token: None,
        expires_in: 0,
        username,
        is_super_admin,
    }))
}

#[derive(serde::Serialize)]
struct MeResponse {
    username: String,
    is_super_admin: bool,
    groups: Vec<UserGroupInfo>,
}

#[derive(serde::Serialize)]
struct UserGroupInfo {
    name: String,
    role: String,
}

async fn get_me(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<MeResponse> {
    let config = state.store.snapshot().await;
    let mut groups = Vec::new();

    for g in &config.groups {
        if user.is_super_admin || g.admins.contains(&user.username) {
            groups.push(UserGroupInfo {
                name: g.name.clone(),
                role: "admin".into(),
            });
        } else if g.members.contains(&user.username) {
            groups.push(UserGroupInfo {
                name: g.name.clone(),
                role: "member".into(),
            });
        }
    }

    Json(MeResponse {
        username: user.username,
        is_super_admin: user.is_super_admin,
        groups,
    })
}

// --------------- Config ---------------

async fn get_config(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
) -> Json<Arc<MockConfig>> {
    Json(state.store.snapshot().await)
}

async fn put_config(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(config): Json<MockConfig>,
) -> Result<Json<Arc<MockConfig>>, AppError> {
    require_super_admin(&user)?;

    for service in &config.services {
        if let Err(e) = validate_service(service) {
            tracing::warn!(service = %service.name, field = %e.field, reason = %e.message, "config rejected: invalid service");
            return Err(AppError::Validation(e.message));
        }
    }
    state.store.replace(config).await.map_err(AppError::Store)?;
    Ok(Json(state.store.snapshot().await))
}

async fn reset_config(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    require_super_admin(&user)?;

    tracing::info!(user = %user.username, "config reset: all services removed");
    state
        .store
        .replace(MockConfig::empty())
        .await
        .map_err(AppError::Store)?;
    Ok(StatusCode::NO_CONTENT)
}

// --------------- Logs ---------------

#[derive(serde::Deserialize)]
struct LogsQuery {
    #[serde(default = "default_log_limit")]
    limit: usize,
}
fn default_log_limit() -> usize {
    50
}

async fn get_logs(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Query(q): Query<LogsQuery>,
) -> Json<Vec<LogEntry>> {
    Json(state.request_log.recent(q.limit))
}

// --------------- Services ---------------

async fn list_services(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<Vec<Service>> {
    let config = state.store.snapshot().await;
    Json(visible_services(&user.username, user.is_super_admin, &config))
}

async fn get_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<Json<Service>, AppError> {
    let config = state.store.snapshot().await;
    let service = config
        .services
        .iter()
        .find(|s| s.name == name)
        .ok_or(AppError::NotFound)?;

    if state.auth_config.enabled
        && !can_access_service(&user.username, user.is_super_admin, service, &config.groups)
    {
        return Err(AppError::Forbidden);
    }

    Ok(Json(service.clone()))
}

async fn create_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(service): Json<Service>,
) -> Result<(StatusCode, Json<Service>), AppError> {
    if state.auth_config.enabled && !user.is_super_admin {
        if let Some(ref gn) = service.group_name {
            let config = state.store.snapshot().await;
            let group = config.groups.iter().find(|g| &g.name == gn);
            match group {
                Some(g) if !can_manage_group(&user.username, false, g) => {
                    return Err(AppError::Forbidden);
                }
                None => {
                    return Err(AppError::Validation(format!(
                        "Le groupe \"{gn}\" n'existe pas."
                    )));
                }
                _ => {}
            }
        } else {
            return Err(AppError::Forbidden);
        }
    }

    if let Err(e) = validate_service(&service) {
        tracing::warn!(service = %service.name, field = %e.field, reason = %e.message, "service rejected");
        return Err(AppError::Validation(e.message));
    }

    {
        let config = state.store.snapshot().await;
        if config.services.iter().any(|s| s.name == service.name) {
            tracing::warn!(service = %service.name, "service creation refused: name already exists");
            return Err(AppError::Conflict(format!(
                "Un service avec le nom \"{}\" existe deja.",
                service.name
            )));
        }
    }

    let updated = state
        .store
        .update(|cfg| {
            cfg.services.push(service.clone());
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .iter()
        .find(|s| s.name == service.name)
        .cloned()
        .map(|s| (StatusCode::CREATED, Json(s)))
        .ok_or(AppError::NotFound)
}

async fn update_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(service): Json<Service>,
) -> Result<Json<Service>, AppError> {
    {
        let config = state.store.snapshot().await;
        let existing = config
            .services
            .iter()
            .find(|s| s.name == name)
            .ok_or(AppError::NotFound)?;

        if state.auth_config.enabled
            && !can_access_service(
                &user.username,
                user.is_super_admin,
                existing,
                &config.groups,
            )
        {
            return Err(AppError::Forbidden);
        }
    }

    if let Err(e) = validate_service(&service) {
        tracing::warn!(service = %name, field = %e.field, reason = %e.message, "service rejected");
        return Err(AppError::Validation(e.message));
    }

    let updated = state
        .store
        .update(|cfg| {
            if let Some(existing) = cfg.services.iter_mut().find(|s| s.name == name) {
                *existing = service.clone();
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .iter()
        .find(|s| s.name == service.name)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn delete_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    if state.auth_config.enabled {
        require_super_admin(&user)?;
    }

    let updated = state
        .store
        .update(|cfg| {
            cfg.services.retain(|s| s.name != name);
        })
        .await
        .map_err(AppError::Store)?;

    if updated.services.iter().any(|s| s.name == name) {
        Err(AppError::NotFound)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

#[derive(serde::Deserialize)]
struct TogglePayload {
    is_mocked: bool,
}

async fn toggle_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(payload): Json<TogglePayload>,
) -> Result<Json<Service>, AppError> {
    {
        let config = state.store.snapshot().await;
        let svc = config
            .services
            .iter()
            .find(|s| s.name == name)
            .ok_or(AppError::NotFound)?;

        if state.auth_config.enabled
            && !can_access_service(&user.username, user.is_super_admin, svc, &config.groups)
        {
            return Err(AppError::Forbidden);
        }
    }

    let updated = state
        .store
        .update(|cfg| {
            if let Some(svc) = cfg.services.iter_mut().find(|s| s.name == name) {
                svc.is_mocked = payload.is_mocked;
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .iter()
        .find(|s| s.name == name)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[derive(serde::Deserialize)]
struct ReorderPayload {
    order: Vec<String>,
}

async fn reorder_rules(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(payload): Json<ReorderPayload>,
) -> Result<Json<Service>, AppError> {
    {
        let config = state.store.snapshot().await;
        let svc = config
            .services
            .iter()
            .find(|s| s.name == name)
            .ok_or(AppError::NotFound)?;

        if state.auth_config.enabled
            && !can_access_service(&user.username, user.is_super_admin, svc, &config.groups)
        {
            return Err(AppError::Forbidden);
        }
    }

    let updated = state
        .store
        .update(|cfg| {
            if let Some(svc) = cfg.services.iter_mut().find(|s| s.name == name) {
                let mut reordered = Vec::with_capacity(svc.rules.len());
                for rule_name in &payload.order {
                    if let Some(pos) = svc.rules.iter().position(|r| &r.name == rule_name) {
                        reordered.push(svc.rules.remove(pos));
                    }
                }
                reordered.append(&mut svc.rules);
                svc.rules = reordered;
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .iter()
        .find(|s| s.name == name)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

// --------------- Groups ---------------

async fn list_groups(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<Vec<Group>> {
    let config = state.store.snapshot().await;
    if user.is_super_admin {
        return Json(config.groups.clone());
    }
    let visible: Vec<Group> = config
        .groups
        .iter()
        .filter(|g| {
            g.admins.contains(&user.username) || g.members.contains(&user.username)
        })
        .cloned()
        .collect();
    Json(visible)
}

async fn get_group(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<Json<Group>, AppError> {
    let config = state.store.snapshot().await;
    let group = config
        .groups
        .iter()
        .find(|g| g.name == name)
        .ok_or(AppError::NotFound)?;

    if state.auth_config.enabled
        && !user.is_super_admin
        && !group.admins.contains(&user.username)
        && !group.members.contains(&user.username)
    {
        return Err(AppError::Forbidden);
    }

    Ok(Json(group.clone()))
}

async fn create_group(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(mut group): Json<Group>,
) -> Result<(StatusCode, Json<Group>), AppError> {
    let name = group.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "Le nom du groupe est requis.".into(),
        ));
    }
    group.name = name;

    {
        let config = state.store.snapshot().await;
        if config
            .groups
            .iter()
            .any(|g| g.name.eq_ignore_ascii_case(&group.name))
        {
            return Err(AppError::Conflict(format!(
                "Un groupe avec le nom \"{}\" existe deja.",
                group.name
            )));
        }

        let existing_codes: Vec<String> = config.groups.iter().map(|g| g.code.clone()).collect();
        let code = if group.code.trim().is_empty() {
            crate::server::codegen::generate_code(&group.name, &existing_codes)
        } else {
            let c = group.code.trim().to_lowercase();
            if c.len() != 5 || !c.chars().all(|ch| ch.is_ascii_alphanumeric()) {
                return Err(AppError::Validation(
                    "Le code groupe doit faire exactement 5 caracteres alphanumeriques.".into(),
                ));
            }
            if config.groups.iter().any(|g| g.code.eq_ignore_ascii_case(&c)) {
                return Err(AppError::Conflict(format!(
                    "Le code \"{c}\" est deja utilise par un autre groupe."
                )));
            }
            c
        };
        group.code = code;
    }

    if !group.admins.contains(&user.username) {
        group.admins.push(user.username.clone());
    }

    let updated = state
        .store
        .update(|cfg| {
            cfg.groups.push(group.clone());
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .groups
        .iter()
        .find(|g| g.name == group.name)
        .cloned()
        .map(|g| (StatusCode::CREATED, Json(g)))
        .ok_or(AppError::NotFound)
}

async fn update_group(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(group): Json<Group>,
) -> Result<Json<Group>, AppError> {
    {
        let config = state.store.snapshot().await;
        let existing = config.groups.iter().find(|g| g.name == name).ok_or(AppError::NotFound)?;
        if state.auth_config.enabled && !can_manage_group(&user.username, user.is_super_admin, existing) {
            return Err(AppError::Forbidden);
        }
    }

    let updated = state
        .store
        .update(|cfg| {
            if let Some(existing) = cfg.groups.iter_mut().find(|g| g.name == name) {
                *existing = group.clone();
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .groups
        .iter()
        .find(|g| g.name == group.name)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn delete_group(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    {
        let config = state.store.snapshot().await;
        let existing = config.groups.iter().find(|g| g.name == name).ok_or(AppError::NotFound)?;
        if state.auth_config.enabled && !can_manage_group(&user.username, user.is_super_admin, existing) {
            return Err(AppError::Forbidden);
        }
    }

    {
        let config = state.store.snapshot().await;
        if config
            .services
            .iter()
            .any(|s| s.group_name.as_deref() == Some(&name))
        {
            return Err(AppError::Conflict(format!(
                "Impossible de supprimer le groupe \"{name}\" : des services y sont encore associes."
            )));
        }
    }

    state
        .store
        .update(|cfg| {
            cfg.groups.retain(|g| g.name != name);
        })
        .await
        .map_err(AppError::Store)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
struct UpdateMembersPayload {
    #[serde(default)]
    admins: Vec<String>,
    #[serde(default)]
    members: Vec<String>,
}

async fn update_group_members(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(payload): Json<UpdateMembersPayload>,
) -> Result<Json<Group>, AppError> {
    {
        let config = state.store.snapshot().await;
        let group = config
            .groups
            .iter()
            .find(|g| g.name == name)
            .ok_or(AppError::NotFound)?;

        if state.auth_config.enabled
            && !can_manage_group(&user.username, user.is_super_admin, group)
        {
            return Err(AppError::Forbidden);
        }
    }

    let updated = state
        .store
        .update(|cfg| {
            if let Some(g) = cfg.groups.iter_mut().find(|g| g.name == name) {
                g.admins = payload.admins.clone();
                g.members = payload.members.clone();
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .groups
        .iter()
        .find(|g| g.name == name)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

// --------------- Helpers & Errors ---------------


fn require_super_admin(user: &AuthUser) -> Result<(), AppError> {
    if !user.is_super_admin {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

enum AppError {
    Store(crate::store::StoreError),
    NotFound,
    Validation(String),
    Conflict(String),
    Unauthorized,
    Forbidden,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::Store(e) => {
                tracing::error!(error = %e, "store error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
            AppError::Validation(msg) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Identifiants invalides" })),
            )
                .into_response(),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Acces refuse" })),
            )
                .into_response(),
        }
    }
}
