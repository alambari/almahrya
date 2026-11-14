use crate::auth::{Token, require_token};
use crate::error::ApiFailure;
use crate::ws::events;
use almahrya_api_types::{CreateSessionRequest, HealthDto, SessionDto, SessionKindDto};
use almahrya_core::SessionKind;
use almahrya_engine::Engine;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn router(engine: Arc<Engine>, token: Token) -> Router {
    let protected = Router::new()
        .route("/v1/sessions", get(list_sessions).post(create_session))
        .route("/v1/events", get(events))
        .route_layer(from_fn_with_state(token, require_token))
        .with_state(engine);
    Router::new()
        .route("/v1/health", get(health))
        .merge(protected)
}

async fn health() -> Json<HealthDto> {
    Json(HealthDto {
        status: "ok".to_owned(),
        pid: std::process::id(),
    })
}

async fn list_sessions(
    State(engine): State<Arc<Engine>>,
) -> Result<Json<Vec<SessionDto>>, ApiFailure> {
    let sessions = engine.list_sessions()?;
    Ok(Json(sessions.iter().map(SessionDto::from).collect()))
}

async fn create_session(
    State(engine): State<Arc<Engine>>,
    body: Result<Json<CreateSessionRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<SessionDto>), ApiFailure> {
    let Json(request) = body.map_err(|e| ApiFailure::InvalidRequest(e.body_text()))?;
    let kind = match request.kind {
        SessionKindDto::Chat => SessionKind::Chat,
        SessionKindDto::Ticket { key } => SessionKind::Ticket { key },
    };
    let session = engine.create_session(request.title, kind).await?;
    Ok((StatusCode::CREATED, Json(SessionDto::from(&session))))
}
