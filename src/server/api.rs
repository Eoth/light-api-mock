use crate::auth::middleware::AuthUser;
use crate::auth::{can_access_service, can_manage_group, visible_services};
use crate::engine::matcher::{
    ConditionEvaluation, ConflictWinner, MatchEngine, OtherRuleConflictInput, RequestData,
    RuleConflictDraft, RuleTestInput,
};
use crate::models::{ConditionGroup, Group, MockConfig, Service};
use crate::server::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use crate::server::request_log::LogEntry;
use crate::server::validation::{validate_backup_filename, validate_service};
use axum::Extension;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete as delete_route, get, post, put};
use axum::{Json, Router};

pub fn routes() -> Router<AppState> {
    let router = Router::new()
        .route("/health", get(health))
        .route("/auth/status", get(auth_status))
        .route("/auth/login", post(login))
        .route("/auth/validate", post(validate_token))
        .route("/auth/me", get(get_me))
        .route("/config", get(get_config).put(put_config))
        .route("/config/reset", delete_route(reset_config))
        .route("/config/backups", get(list_backups))
        .route("/config/restore/:filename", post(restore_backup))
        .route("/services", get(list_services).post(create_service))
        .route(
            "/services/:name",
            get(get_service).put(update_service).delete(delete_service),
        )
        .route("/services/:name/toggle", put(toggle_service))
        .route("/services/:name/ping", post(ping_service))
        .route("/services/:name/rules/reorder", put(reorder_rules))
        .route(
            "/groups/:group/services/:name",
            get(get_service_grouped).put(update_service_grouped).delete(delete_service_grouped),
        )
        .route("/groups/:group/services/:name/toggle", put(toggle_service_grouped))
        .route("/groups/:group/services/:name/ping", post(ping_service_grouped))
        .route("/groups/:group/services/:name/rules/reorder", put(reorder_rules_grouped))
        .route("/script/validate", post(validate_script))
        .route("/rule-test", post(test_rule))
        .route("/rule-conflicts", post(check_rule_conflicts))
        .route("/logs", get(get_logs))
        .route("/groups", get(list_groups).post(create_group))
        .route(
            "/groups/:name",
            get(get_group).put(update_group).delete(delete_group),
        )
        .route("/groups/:name/members", put(update_group_members));

    #[cfg(feature = "messaging-kafka")]
    let router = router
        .route("/messaging/status", get(messaging_status))
        .route("/messaging/logs", get(get_messaging_logs))
        .route("/messaging/simulate", post(simulate_message));

    router
}

// --------------- Health ---------------

#[derive(serde::Serialize)]
struct HealthResponse {
    status: &'static str,
    write_queue: crate::store::WriteQueueStatus,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        write_queue: state.store.writer_status(),
    })
}

// --------------- Auth ---------------

#[derive(serde::Serialize)]
struct AuthStatusResponse {
    enabled: bool,
    show_reset_button: bool,
}

async fn auth_status(State(state): State<AppState>) -> Json<AuthStatusResponse> {
    Json(AuthStatusResponse {
        enabled: state.auth_config.enabled,
        show_reset_button: state.auth_config.show_reset_button,
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
    Json(mut config): Json<MockConfig>,
) -> Result<Json<Arc<MockConfig>>, AppError> {
    require_super_admin(&user)?;

    for service in &config.services {
        if let Err(e) = validate_service(service) {
            tracing::warn!(service = %service.name, field = %e.field, reason = %e.message, "config rejected: invalid service");
            return Err(AppError::Validation(e.message));
        }
    }

    ensure_group_codes(&mut config.groups);

    state.store.replace(config).await.map_err(AppError::Store)?;
    Ok(Json(state.store.snapshot().await))
}

async fn reset_config(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    require_super_admin(&user)?;

    state.store.backup_before_reset().await.map_err(AppError::Store)?;

    tracing::info!(user = %user.username, "config reset: all services removed");
    state
        .store
        .replace(MockConfig::empty())
        .await
        .map_err(AppError::Store)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_backups(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<crate::store::BackupInfo>>, AppError> {
    require_super_admin(&user)?;
    let backups = state.store.list_backups().await.map_err(AppError::Store)?;
    Ok(Json(backups))
}

async fn restore_backup(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(filename): Path<String>,
) -> Result<StatusCode, AppError> {
    require_super_admin(&user)?;

    if let Err(e) = validate_backup_filename(&filename) {
        tracing::warn!(filename = %filename, reason = %e.message, "restore rejected: invalid filename");
        return Err(AppError::Validation(e.message));
    }

    tracing::info!(user = %user.username, filename = %filename, "config restore requested");
    state
        .store
        .restore_from_backup(&filename)
        .await
        .map_err(|e| match e {
            crate::store::StoreError::NotFound(_) => AppError::NotFound,
            other => AppError::Store(other),
        })?;
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

// --------------- Messaging (Kafka) ---------------
// Feature-gated : ces routes n'existent meme pas dans un build par defaut
// (cf routes() ci-dessus). Meme garde d'auth que /logs (utilisateur
// authentifie, pas de restriction super-admin — lecture seule + simulation,
// pas d'operation destructive).

#[cfg(feature = "messaging-kafka")]
#[derive(serde::Serialize)]
struct MessagingStatusResponse {
    /// Toujours `true` ici : l'existence meme de la reponse (200, pas 404)
    /// suffit au frontend a detecter que le binaire a ete compile avec la
    /// feature "messaging-kafka" — c'est ce champ qui distingue "route
    /// absente" (binaire sans la feature) de "fonctionnalite compilee".
    available: bool,
}

#[cfg(feature = "messaging-kafka")]
async fn messaging_status() -> Json<MessagingStatusResponse> {
    Json(MessagingStatusResponse { available: true })
}

#[cfg(feature = "messaging-kafka")]
async fn get_messaging_logs(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Query(q): Query<LogsQuery>,
) -> Json<Vec<crate::messaging::message_log::MessageLogEntry>> {
    Json(state.messaging.message_log.recent(q.limit))
}

/// Simule la reception d'un message sur le topic d'ecoute : declenche
/// exactement le meme pipeline (match -> rendu -> journal -> publication
/// eventuelle sur reply_topic) que le vrai consumer Kafka
/// (messaging::consumer::process_message), sans dependre d'un producteur
/// Kafka externe. Utile pour tester une regle de messaging depuis l'UI
/// (bouton "Simuler un message") et pour les tests E2E dans un environnement
/// sans broker Kafka reel.
#[cfg(feature = "messaging-kafka")]
#[derive(serde::Deserialize)]
struct SimulateMessageRequest {
    topic: String,
    #[serde(default)]
    headers: std::collections::HashMap<String, String>,
    payload: String,
}

#[cfg(feature = "messaging-kafka")]
async fn simulate_message(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthUser>,
    Json(req): Json<SimulateMessageRequest>,
) -> StatusCode {
    crate::messaging::consumer::process_message(
        &state.store,
        &state.messaging.message_log,
        state.messaging.reply_topic.as_deref(),
        &state.messaging.publisher,
        &req.topic,
        req.payload.as_bytes(),
        req.headers,
    )
    .await;
    StatusCode::NO_CONTENT
}

// --------------- Services ---------------

async fn list_services(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Json<Vec<Service>> {
    let config = state.store.snapshot().await;
    Json(visible_services(&user.username, user.is_super_admin, &config))
}

// Un service est identifie de facon non ambigue par (group_name, name) : le
// nom seul ne suffit pas puisque `create_service` autorise deux services du
// meme nom dans des groupes differents (cf commit 8fdb9c0, "unicite service
// par groupe, pas globale"). `group` vaut `None` pour le perimetre "sans
// groupe", qui forme son propre espace de noms au meme titre qu'un groupe.
fn service_matches(s: &Service, group: Option<&str>, name: &str) -> bool {
    s.name == name && s.group_name.as_deref() == group
}

async fn get_service_impl(
    state: AppState,
    user: AuthUser,
    group: Option<String>,
    name: String,
) -> Result<Json<Service>, AppError> {
    let config = state.store.snapshot().await;
    let service = config
        .services
        .iter()
        .find(|s| service_matches(s, group.as_deref(), &name))
        .ok_or(AppError::NotFound)?;

    if state.auth_config.enabled
        && !can_access_service(&user.username, user.is_super_admin, service, &config.groups)
    {
        return Err(AppError::Forbidden);
    }

    Ok(Json(service.clone()))
}

async fn get_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<Json<Service>, AppError> {
    get_service_impl(state, user, None, name).await
}

async fn get_service_grouped(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((group, name)): Path<(String, String)>,
) -> Result<Json<Service>, AppError> {
    get_service_impl(state, user, Some(group), name).await
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
        let same_group = config.services.iter().any(|s| {
            s.name == service.name && s.group_name == service.group_name
        });
        if same_group {
            tracing::warn!(service = %service.name, "service creation refused: name already exists in group");
            return Err(AppError::Conflict(format!(
                "Un service avec le nom \"{}\" existe deja dans ce groupe.",
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

async fn update_service_impl(
    state: AppState,
    user: AuthUser,
    group: Option<String>,
    name: String,
    service: Service,
) -> Result<Json<Service>, AppError> {
    {
        let config = state.store.snapshot().await;
        let existing = config
            .services
            .iter()
            .find(|s| service_matches(s, group.as_deref(), &name))
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

    let new_group = service.group_name.clone();
    let new_name = service.name.clone();
    let updated = state
        .store
        .update(|cfg| {
            if let Some(existing) = cfg
                .services
                .iter_mut()
                .find(|s| service_matches(s, group.as_deref(), &name))
            {
                *existing = service.clone();
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .iter()
        .find(|s| service_matches(s, new_group.as_deref(), &new_name))
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn update_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(service): Json<Service>,
) -> Result<Json<Service>, AppError> {
    update_service_impl(state, user, None, name, service).await
}

async fn update_service_grouped(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((group, name)): Path<(String, String)>,
    Json(service): Json<Service>,
) -> Result<Json<Service>, AppError> {
    update_service_impl(state, user, Some(group), name, service).await
}

async fn delete_service_impl(
    state: AppState,
    user: AuthUser,
    group: Option<String>,
    name: String,
) -> Result<StatusCode, AppError> {
    if state.auth_config.enabled {
        require_super_admin(&user)?;
    }

    let updated = state
        .store
        .update(|cfg| {
            cfg.services
                .retain(|s| !service_matches(s, group.as_deref(), &name));
        })
        .await
        .map_err(AppError::Store)?;

    if updated
        .services
        .iter()
        .any(|s| service_matches(s, group.as_deref(), &name))
    {
        Err(AppError::NotFound)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

async fn delete_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    delete_service_impl(state, user, None, name).await
}

async fn delete_service_grouped(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((group, name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    delete_service_impl(state, user, Some(group), name).await
}

#[derive(serde::Deserialize)]
struct TogglePayload {
    is_mocked: bool,
}

async fn toggle_service_impl(
    state: AppState,
    user: AuthUser,
    group: Option<String>,
    name: String,
    payload: TogglePayload,
) -> Result<Json<Service>, AppError> {
    {
        let config = state.store.snapshot().await;
        let svc = config
            .services
            .iter()
            .find(|s| service_matches(s, group.as_deref(), &name))
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
            if let Some(svc) = cfg
                .services
                .iter_mut()
                .find(|s| service_matches(s, group.as_deref(), &name))
            {
                svc.is_mocked = payload.is_mocked;
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .iter()
        .find(|s| service_matches(s, group.as_deref(), &name))
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn toggle_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(payload): Json<TogglePayload>,
) -> Result<Json<Service>, AppError> {
    toggle_service_impl(state, user, None, name, payload).await
}

async fn toggle_service_grouped(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((group, name)): Path<(String, String)>,
    Json(payload): Json<TogglePayload>,
) -> Result<Json<Service>, AppError> {
    toggle_service_impl(state, user, Some(group), name, payload).await
}

async fn ping_service_impl(
    state: AppState,
    user: AuthUser,
    group: Option<String>,
    name: String,
) -> Result<Json<crate::engine::PingStatus>, AppError> {
    let target_url = {
        let config = state.store.snapshot().await;
        let svc = config
            .services
            .iter()
            .find(|s| service_matches(s, group.as_deref(), &name))
            .ok_or(AppError::NotFound)?;

        if state.auth_config.enabled
            && !can_access_service(&user.username, user.is_super_admin, svc, &config.groups)
        {
            return Err(AppError::Forbidden);
        }

        svc.real_target_url.clone()
    };

    // Cache clef par nom seul (pas par groupe) : deux services de meme nom
    // dans des groupes differents auraient des cibles reseau distinctes
    // partageant la meme cle de cache — limitation pre-existante mineure
    // (le pire cas est un badge de disponibilite affichant un resultat en
    // cache pour le mauvais service pendant la TTL de 2 min), non corrigee
    // ici car hors perimetre de ce correctif (ambiguite d'IDENTIFICATION du
    // service cible, pas de son statut de ping affiche).
    if let Some(cached) = state.ping_cache.get_fresh(&name, crate::server::ping::PING_TTL_MS) {
        return Ok(Json(cached));
    }

    let status = state.proxy.ping(&target_url).await;
    state.ping_cache.set(&name, status.clone());
    Ok(Json(status))
}

async fn ping_service(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<Json<crate::engine::PingStatus>, AppError> {
    ping_service_impl(state, user, None, name).await
}

async fn ping_service_grouped(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((group, name)): Path<(String, String)>,
) -> Result<Json<crate::engine::PingStatus>, AppError> {
    ping_service_impl(state, user, Some(group), name).await
}

#[derive(serde::Deserialize)]
struct ReorderPayload {
    order: Vec<String>,
}

async fn reorder_rules_impl(
    state: AppState,
    user: AuthUser,
    group: Option<String>,
    name: String,
    payload: ReorderPayload,
) -> Result<Json<Service>, AppError> {
    {
        let config = state.store.snapshot().await;
        let svc = config
            .services
            .iter()
            .find(|s| service_matches(s, group.as_deref(), &name))
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
            if let Some(svc) = cfg
                .services
                .iter_mut()
                .find(|s| service_matches(s, group.as_deref(), &name))
            {
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
        .find(|s| service_matches(s, group.as_deref(), &name))
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn reorder_rules(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(payload): Json<ReorderPayload>,
) -> Result<Json<Service>, AppError> {
    reorder_rules_impl(state, user, None, name, payload).await
}

async fn reorder_rules_grouped(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((group, name)): Path<(String, String)>,
    Json(payload): Json<ReorderPayload>,
) -> Result<Json<Service>, AppError> {
    reorder_rules_impl(state, user, Some(group), name, payload).await
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

    state
        .store
        .update(|cfg| {
            for svc in cfg.services.iter_mut() {
                if svc.group_name.as_deref() == Some(&name) {
                    svc.group_name = None;
                }
            }
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


// --------------- Script validation ---------------

#[derive(serde::Deserialize)]
struct ValidateScriptRequest {
    script: String,
}

#[derive(serde::Serialize)]
struct ValidateScriptResponse {
    valid: bool,
    error: Option<String>,
}

async fn validate_script(
    State(state): State<AppState>,
    Json(req): Json<ValidateScriptRequest>,
) -> Json<ValidateScriptResponse> {
    match state.script_engine.validate(&req.script) {
        Ok(()) => Json(ValidateScriptResponse { valid: true, error: None }),
        Err(e) => Json(ValidateScriptResponse { valid: false, error: Some(e) }),
    }
}

// --------------- Testeur de regle (rejeu en lecture seule) ---------------
//
// Endpoint totalement stateless : aucun acces au store, aucune reference a un
// service persiste. Le frontend envoie le brouillon de regle EN COURS
// D'EDITION (pas necessairement sauvegarde) et le detail d'une requete deja
// capturee (RequestLog::CapturedRequest, cf src/server/request_log.rs). Le
// handler ne fait que deserialiser, reconstruire un RequestData, deleguer a
// MatchEngine::evaluate_rule_test, puis serialiser le resultat — aucune
// mutation, aucun appel reseau/proxy, un vrai rejeu local en lecture seule.
//
// Garde d'auth : utilisateur authentifie requis, MEME garde que /logs (pas de
// can_access_service supplementaire) — /logs lui-meme n'est pas scope par
// service aujourd'hui, on ne cree pas ici une incoherence de modele de
// securite pour ce seul endpoint. Pas de variante flat/groupee non plus :
// aucun service n'est charge depuis le store, donc pas d'identite de service
// a desambiguiser (cf service_matches, points 40-41 CLAUDE.md).

#[derive(serde::Deserialize)]
struct RuleTestCapturedRequest {
    method: String,
    remaining_path: String,
    #[serde(default)]
    path_params: HashMap<String, String>,
    #[serde(default)]
    query_params: HashMap<String, String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    body_truncated: bool,
    #[serde(default)]
    content_type: Option<String>,
}

#[derive(serde::Deserialize)]
struct RuleTestRequest {
    method: String,
    sub_path: Option<String>,
    conditions: ConditionGroup,
    request: RuleTestCapturedRequest,
}

#[derive(serde::Serialize)]
struct RuleTestResponse {
    method_matches: bool,
    sub_path_matches: bool,
    path_params: HashMap<String, String>,
    overall_matched: bool,
    body_truncated: bool,
    all_of: Vec<ConditionEvaluation>,
    any_of: Vec<ConditionEvaluation>,
}

async fn test_rule(
    Extension(_user): Extension<AuthUser>,
    Json(payload): Json<RuleTestRequest>,
) -> Json<RuleTestResponse> {
    let body_truncated = payload.request.body_truncated;
    let req = RequestData {
        query_params: payload.request.query_params,
        headers: payload.request.headers,
        body: payload.request.body.into_bytes(),
        content_type: payload.request.content_type,
        path_params: payload.request.path_params,
        method: payload.request.method,
        remaining_path: payload.request.remaining_path,
    };

    let outcome = MatchEngine::evaluate_rule_test(
        RuleTestInput {
            method: &payload.method,
            sub_path: &payload.sub_path,
            conditions: &payload.conditions,
        },
        &req,
    );

    Json(RuleTestResponse {
        method_matches: outcome.method_matches,
        sub_path_matches: outcome.sub_path_matches,
        path_params: outcome.path_params,
        overall_matched: outcome.overall_matched,
        body_truncated,
        all_of: outcome.group.all_of,
        any_of: outcome.group.any_of,
    })
}

// --------------- Detecteur de conflit entre regles (a la sauvegarde) ---------------
//
// Endpoint stateless, meme famille que /api/rule-test ci-dessus : aucun acces
// au store, aucune reference a un service persiste. Le frontend envoie le
// brouillon de regle EN COURS DE SAUVEGARDE (RuleForm, avant l'appel PUT
// /api/services/:name qui persiste reellement), la liste des AUTRES regles
// du service dans leur ordre actuel, et la position ou le brouillon se
// retrouvera une fois sauvegarde. Le handler ne fait que deserialiser,
// deleguer a MatchEngine::find_rule_conflicts, serialiser le resultat —
// aucune mutation, purement informatif (cf CLAUDE.md pour le detail de
// l'algorithme et ses limites assumees).
//
// Garde d'auth : utilisateur authentifie requis, MEME garde que /rule-test
// et /logs — pas de can_access_service supplementaire, cette route ne
// charge aucun service depuis le store donc pas d'identite de service a
// desambiguiser (cf service_matches, points 40-41 CLAUDE.md).

#[derive(serde::Deserialize)]
struct RuleConflictDraftRequest {
    method: String,
    sub_path: Option<String>,
    conditions: ConditionGroup,
}

#[derive(serde::Deserialize)]
struct OtherRuleConflictRequest {
    name: String,
    method: String,
    sub_path: Option<String>,
    conditions: ConditionGroup,
}

#[derive(serde::Deserialize)]
struct RuleConflictsRequest {
    draft: RuleConflictDraftRequest,
    other_rules: Vec<OtherRuleConflictRequest>,
    draft_position: usize,
}

#[derive(serde::Serialize)]
struct RuleConflictResponseItem {
    other_rule_name: String,
    winner: ConflictWinner,
}

#[derive(serde::Serialize)]
struct RuleConflictsResponse {
    conflicts: Vec<RuleConflictResponseItem>,
}

async fn check_rule_conflicts(
    Extension(_user): Extension<AuthUser>,
    Json(payload): Json<RuleConflictsRequest>,
) -> Json<RuleConflictsResponse> {
    let draft = RuleConflictDraft {
        method: &payload.draft.method,
        sub_path: &payload.draft.sub_path,
        conditions: &payload.draft.conditions,
    };
    let other_rules: Vec<OtherRuleConflictInput> = payload
        .other_rules
        .iter()
        .map(|r| OtherRuleConflictInput {
            name: &r.name,
            method: &r.method,
            sub_path: &r.sub_path,
            conditions: &r.conditions,
        })
        .collect();

    let conflicts = MatchEngine::find_rule_conflicts(&draft, &other_rules, payload.draft_position);

    Json(RuleConflictsResponse {
        conflicts: conflicts
            .into_iter()
            .map(|c| RuleConflictResponseItem {
                other_rule_name: c.other_rule_name,
                winner: c.winner,
            })
            .collect(),
    })
}

fn ensure_group_codes(groups: &mut Vec<Group>) {
    let mut existing_codes: Vec<String> = groups
        .iter()
        .filter(|g| !g.code.trim().is_empty())
        .map(|g| g.code.clone())
        .collect();
    for group in groups.iter_mut() {
        if group.code.trim().is_empty() {
            let code = crate::server::codegen::generate_code(&group.name, &existing_codes);
            group.code = code.clone();
            existing_codes.push(code);
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // require_super_admin() est le seul garde-fou reel derriere reset_config
    // ET restore_backup (cf CLAUDE.md) : verifie directement ici plutot que
    // via un test HTTP bout-en-bout (pas d'infra de test router dans ce
    // fichier a ce jour).
    #[test]
    fn require_super_admin_rejects_non_admin() {
        let user = AuthUser { username: "bob".into(), is_super_admin: false };
        assert!(matches!(require_super_admin(&user), Err(AppError::Forbidden)));
    }

    #[test]
    fn require_super_admin_accepts_admin() {
        let user = AuthUser { username: "alice".into(), is_super_admin: true };
        assert!(require_super_admin(&user).is_ok());
    }

    // service_matches() est la seule fonction qui decide de l'identite d'un
    // service (name + group_name) — utilisee par TOUS les handlers scopes
    // (get/update/delete/toggle/ping/reorder). Ces tests couvrent directement
    // la portee reelle de l'unicite (par groupe, pas globale) sans avoir
    // besoin d'un serveur HTTP complet.
    fn svc_named(name: &str, group: Option<&str>) -> Service {
        Service {
            name: name.into(),
            listen_path: "".into(),
            real_target_url: "http://example.com".into(),
            is_mocked: true,
            rewrite_directory_urls: false,
            group_name: group.map(|g| g.to_string()),
            wsdl_mode: crate::models::WsdlMode::default(),
            rules: vec![],
        }
    }

    #[test]
    fn service_matches_same_name_same_group() {
        let s = svc_named("foo", Some("team-a"));
        assert!(service_matches(&s, Some("team-a"), "foo"));
    }

    #[test]
    fn service_matches_same_name_different_group_does_not_match() {
        let s = svc_named("foo", Some("team-a"));
        assert!(!service_matches(&s, Some("team-b"), "foo"));
    }

    #[test]
    fn service_matches_ungrouped_is_its_own_scope() {
        let grouped = svc_named("foo", Some("team-a"));
        let ungrouped = svc_named("foo", None);
        assert!(!service_matches(&grouped, None, "foo"));
        assert!(service_matches(&ungrouped, None, "foo"));
    }

    #[test]
    fn service_matches_different_name_never_matches() {
        let s = svc_named("foo", Some("team-a"));
        assert!(!service_matches(&s, Some("team-a"), "bar"));
    }

    // --- Infra de test HTTP minimale (reprend le pattern deja etabli dans
    // server::intercept::tests : vrai routeur Axum + vrai listener TCP,
    // plutot qu'un style tower::oneshot qui n'existe pas encore dans ce
    // projet). Justifie ici par la gravite du bug couvert (suppression
    // croisee entre groupes) : une regression doit etre detectee par
    // `cargo test` seul, sans dependre de la suite Playwright.
    async fn spawn_test_app(config: MockConfig) -> String {
        let data_dir = std::env::temp_dir().join(format!(
            "lightmock-api-test-{}",
            fastrand::u64(..)
        ));
        std::fs::create_dir_all(&data_dir).unwrap();
        let store = crate::store::MockStore::new(data_dir.join("mock-config.yaml"));
        store.replace(config).await.unwrap();
        store.flush().await;

        #[cfg(feature = "messaging-kafka")]
        let messaging = crate::messaging::MessagingState {
            message_log: crate::messaging::message_log::MessageLog::new(),
            reply_topic: None,
            publisher: crate::messaging::consumer::Publisher::None,
        };
        let state = AppState {
            store,
            proxy: crate::engine::ProxyClient::new(),
            seq_counters: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            request_log: crate::server::request_log::RequestLog::new(),
            auth_config: crate::auth::AuthConfig {
                enabled: false,
                keycloak_url: String::new(),
                realm: String::new(),
                client_id: String::new(),
                super_admins: vec![],
                show_reset_button: false,
            },
            keycloak: None,
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

    fn ambiguous_services() -> Vec<Service> {
        vec![
            svc_named("shared-name", Some("team-a")),
            svc_named("shared-name", Some("team-b")),
            svc_named("shared-name", None),
        ]
    }

    #[tokio::test]
    async fn get_service_grouped_route_returns_only_the_matching_group() {
        let base = spawn_test_app(MockConfig { services: ambiguous_services(), groups: vec![] }).await;
        let client = reqwest::Client::new();

        let via_group_a = client
            .get(format!("{base}/groups/team-a/services/shared-name"))
            .send()
            .await
            .unwrap();
        assert_eq!(via_group_a.status(), 200);
        let svc: Service = via_group_a.json().await.unwrap();
        assert_eq!(svc.group_name.as_deref(), Some("team-a"));

        let via_flat = client
            .get(format!("{base}/services/shared-name"))
            .send()
            .await
            .unwrap();
        assert_eq!(via_flat.status(), 200);
        let svc: Service = via_flat.json().await.unwrap();
        assert_eq!(svc.group_name, None, "la route non scopee doit resoudre au service SANS groupe, pas au premier trouve");
    }

    #[tokio::test]
    async fn delete_ambiguous_service_only_removes_the_targeted_group() {
        let base = spawn_test_app(MockConfig { services: ambiguous_services(), groups: vec![] }).await;
        let client = reqwest::Client::new();

        let del = client
            .delete(format!("{base}/groups/team-a/services/shared-name"))
            .send()
            .await
            .unwrap();
        assert_eq!(del.status(), 204);

        let still_team_b = client
            .get(format!("{base}/groups/team-b/services/shared-name"))
            .send()
            .await
            .unwrap();
        assert_eq!(still_team_b.status(), 200, "le service de team-b ne doit pas avoir ete supprime");

        let still_ungrouped = client
            .get(format!("{base}/services/shared-name"))
            .send()
            .await
            .unwrap();
        assert_eq!(still_ungrouped.status(), 200, "le service sans groupe ne doit pas avoir ete supprime");

        let gone_team_a = client
            .get(format!("{base}/groups/team-a/services/shared-name"))
            .send()
            .await
            .unwrap();
        assert_eq!(gone_team_a.status(), 404);
    }

    #[tokio::test]
    async fn update_via_flat_route_does_not_touch_grouped_namesakes() {
        let base = spawn_test_app(MockConfig { services: ambiguous_services(), groups: vec![] }).await;
        let client = reqwest::Client::new();

        let mut updated = svc_named("shared-name", None);
        updated.real_target_url = "http://changed.example.com".into();
        let put = client
            .put(format!("{base}/services/shared-name"))
            .json(&updated)
            .send()
            .await
            .unwrap();
        assert_eq!(put.status(), 200);

        let team_a = client
            .get(format!("{base}/groups/team-a/services/shared-name"))
            .send()
            .await
            .unwrap()
            .json::<Service>()
            .await
            .unwrap();
        assert_eq!(team_a.real_target_url, "http://example.com", "team-a ne doit pas avoir ete modifie par un PUT scope sans groupe");
    }

    // --- test_rule() : testeur de regle, endpoint stateless ---

    fn anon_user() -> AuthUser {
        AuthUser::anonymous()
    }

    fn empty_captured(method: &str, remaining_path: &str) -> RuleTestCapturedRequest {
        RuleTestCapturedRequest {
            method: method.into(),
            remaining_path: remaining_path.into(),
            path_params: HashMap::new(),
            query_params: HashMap::new(),
            headers: HashMap::new(),
            body: String::new(),
            body_truncated: false,
            content_type: None,
        }
    }

    #[tokio::test]
    async fn test_rule_nominal_match_via_path_param() {
        let mut captured = empty_captured("GET", "/orders/42");
        captured.path_params.clear();
        let payload = RuleTestRequest {
            method: "GET".into(),
            sub_path: Some("/orders/{id}".into()),
            conditions: ConditionGroup {
                all_of: vec![crate::models::Condition {
                    source: crate::models::ConditionSource::PathParam("id".into()),
                    operator: crate::models::Operator::Eq("42".into()),
                }],
                any_of: vec![],
            },
            request: captured,
        };
        let Json(result) = test_rule(Extension(anon_user()), Json(payload)).await;
        assert!(result.method_matches);
        assert!(result.sub_path_matches);
        assert!(result.overall_matched);
        assert_eq!(result.path_params.get("id").unwrap(), "42");
        assert!(result.all_of[0].matched);
    }

    #[tokio::test]
    async fn test_rule_reports_cross_source_hint_for_misplaced_query_param() {
        let mut captured = empty_captured("GET", "/orders/42");
        captured.path_params.insert("id".into(), "42".into());
        let payload = RuleTestRequest {
            method: "GET".into(),
            sub_path: None,
            conditions: ConditionGroup {
                all_of: vec![crate::models::Condition {
                    source: crate::models::ConditionSource::QueryParam("id".into()),
                    operator: crate::models::Operator::Eq("42".into()),
                }],
                any_of: vec![],
            },
            request: captured,
        };
        let Json(result) = test_rule(Extension(anon_user()), Json(payload)).await;
        assert!(!result.overall_matched);
        assert!(!result.all_of[0].matched);
        let hint = result.all_of[0].hint.as_deref().unwrap();
        assert!(hint.contains("parametre de chemin"));
    }

    #[tokio::test]
    async fn test_rule_method_mismatch_reported() {
        let payload = RuleTestRequest {
            method: "GET".into(),
            sub_path: None,
            conditions: ConditionGroup::default(),
            request: empty_captured("POST", "/anything"),
        };
        let Json(result) = test_rule(Extension(anon_user()), Json(payload)).await;
        assert!(!result.method_matches);
        assert!(!result.overall_matched);
    }

    #[tokio::test]
    async fn test_rule_propagates_body_truncated_flag() {
        let mut captured = empty_captured("GET", "/x");
        captured.body_truncated = true;
        let payload = RuleTestRequest {
            method: "GET".into(),
            sub_path: None,
            conditions: ConditionGroup::default(),
            request: captured,
        };
        let Json(result) = test_rule(Extension(anon_user()), Json(payload)).await;
        assert!(result.body_truncated);
    }

    // --- check_rule_conflicts (POST /api/rule-conflicts) tests ---

    fn header_eq_cond(key: &str, val: &str) -> crate::models::Condition {
        crate::models::Condition {
            source: crate::models::ConditionSource::Header(key.into()),
            operator: crate::models::Operator::Eq(val.into()),
        }
    }

    #[tokio::test]
    async fn check_rule_conflicts_reports_identical_conditions() {
        let payload = RuleConflictsRequest {
            draft: RuleConflictDraftRequest {
                method: "GET".into(),
                sub_path: None,
                conditions: ConditionGroup {
                    all_of: vec![header_eq_cond("x-env", "prod")],
                    any_of: vec![],
                },
            },
            other_rules: vec![OtherRuleConflictRequest {
                name: "existing-rule".into(),
                method: "GET".into(),
                sub_path: None,
                conditions: ConditionGroup {
                    all_of: vec![header_eq_cond("x-env", "prod")],
                    any_of: vec![],
                },
            }],
            draft_position: 1,
        };
        let Json(result) = check_rule_conflicts(Extension(anon_user()), Json(payload)).await;
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].other_rule_name, "existing-rule");
        assert_eq!(result.conflicts[0].winner, ConflictWinner::Other);
    }

    #[tokio::test]
    async fn check_rule_conflicts_no_conflict_for_disjoint_conditions() {
        let payload = RuleConflictsRequest {
            draft: RuleConflictDraftRequest {
                method: "GET".into(),
                sub_path: None,
                conditions: ConditionGroup {
                    all_of: vec![header_eq_cond("x-env", "prod")],
                    any_of: vec![],
                },
            },
            other_rules: vec![OtherRuleConflictRequest {
                name: "staging-rule".into(),
                method: "GET".into(),
                sub_path: None,
                conditions: ConditionGroup {
                    all_of: vec![header_eq_cond("x-env", "staging")],
                    any_of: vec![],
                },
            }],
            draft_position: 1,
        };
        let Json(result) = check_rule_conflicts(Extension(anon_user()), Json(payload)).await;
        assert!(result.conflicts.is_empty());
    }

    #[tokio::test]
    async fn check_rule_conflicts_reports_subset_conditions_with_draft_winner() {
        // Le brouillon est repositionne AVANT l'autre regle (edition en
        // place a l'index 0) : c'est donc le brouillon qui gagnerait.
        let payload = RuleConflictsRequest {
            draft: RuleConflictDraftRequest {
                method: "POST".into(),
                sub_path: None,
                conditions: ConditionGroup::default(),
            },
            other_rules: vec![OtherRuleConflictRequest {
                name: "more-specific-rule".into(),
                method: "POST".into(),
                sub_path: None,
                conditions: ConditionGroup {
                    all_of: vec![header_eq_cond("x-env", "prod")],
                    any_of: vec![],
                },
            }],
            draft_position: 0,
        };
        let Json(result) = check_rule_conflicts(Extension(anon_user()), Json(payload)).await;
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].winner, ConflictWinner::Draft);
    }
}
