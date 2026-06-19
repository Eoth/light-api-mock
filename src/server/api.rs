use crate::models::{MockConfig, Service};
use crate::server::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router};
use axum::routing::{get, put};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_config).put(put_config))
        .route("/services", get(list_services))
        .route("/services/:name", get(get_service).put(put_service).delete(delete_service))
        .route("/services/:name/toggle", put(toggle_service))
        .route("/services/:name/rules/reorder", put(reorder_rules))
}

async fn get_config(State(state): State<AppState>) -> Json<MockConfig> {
    Json(state.store.snapshot().await)
}

async fn put_config(
    State(state): State<AppState>,
    Json(config): Json<MockConfig>,
) -> Result<Json<MockConfig>, AppError> {
    state.store.replace(config).await.map_err(AppError::Store)?;
    Ok(Json(state.store.snapshot().await))
}

async fn list_services(State(state): State<AppState>) -> Json<Vec<Service>> {
    Json(state.store.snapshot().await.services)
}

async fn get_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Service>, StatusCode> {
    let config = state.store.snapshot().await;
    config
        .services
        .into_iter()
        .find(|s| s.name == name)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn put_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(service): Json<Service>,
) -> Result<Json<Service>, AppError> {
    let updated = state
        .store
        .update(|cfg| {
            if let Some(existing) = cfg.services.iter_mut().find(|s| s.name == name) {
                *existing = service.clone();
            } else {
                cfg.services.push(service.clone());
            }
        })
        .await
        .map_err(AppError::Store)?;

    updated
        .services
        .into_iter()
        .find(|s| s.name == service.name)
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn delete_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
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
    Path(name): Path<String>,
    Json(payload): Json<TogglePayload>,
) -> Result<Json<Service>, AppError> {
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
        .into_iter()
        .find(|s| s.name == name)
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[derive(serde::Deserialize)]
struct ReorderPayload {
    order: Vec<String>,
}

async fn reorder_rules(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<ReorderPayload>,
) -> Result<Json<Service>, AppError> {
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
        .into_iter()
        .find(|s| s.name == name)
        .map(Json)
        .ok_or(AppError::NotFound)
}

enum AppError {
    Store(crate::store::StoreError),
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::Store(e) => {
                tracing::error!(error = %e, "store error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}
