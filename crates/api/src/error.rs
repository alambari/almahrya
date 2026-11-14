use almahrya_api_types::ApiError;
use almahrya_engine::EngineError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Every way a request can fail, rendered as an [`ApiError`] body.
#[derive(Debug, thiserror::Error)]
pub enum ApiFailure {
    #[error("missing or invalid token")]
    Unauthorized,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Engine(#[from] EngineError),
}

impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Missing or invalid token".to_owned(),
            ),
            Self::InvalidRequest(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                message.clone(),
            ),
            // Internals stay out of the response body.
            Self::Engine(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal error".to_owned(),
            ),
        };
        let body = ApiError {
            code: code.to_owned(),
            message,
        };
        (status, Json(body)).into_response()
    }
}
