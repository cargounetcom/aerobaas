use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use aero_protocol::AeroError;

/// API error. Body: `{"error": "<code>", "message": "<text>", "status": <http status>}`
/// (same `error`/`message` fields as the SovereignBaaS admin API).
pub enum AeroApiError {
    Aero(AeroError),
    BadRequest(String),
}

impl From<AeroError> for AeroApiError {
    fn from(e: AeroError) -> Self {
        Self::Aero(e)
    }
}

impl From<anyhow::Error> for AeroApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::Aero(AeroError::Internal(e.to_string()))
    }
}

impl IntoResponse for AeroApiError {
    fn into_response(self) -> Response {
        let e = match self {
            Self::Aero(e) => e,
            Self::BadRequest(m) => AeroError::BadRequest(m),
        };
        let status = e.status_code();
        let message = match &e {
            // Never leak internals to clients; the details are in the server log.
            AeroError::Internal(detail) => {
                tracing::error!(error = %detail, "internal error");
                "Internal server error".to_string()
            }
            AeroError::BadRequest(m) => m.clone(),
            AeroError::Forbidden(m)
            | AeroError::Unauthorized(m)
            | AeroError::NotFound(m)
            | AeroError::Conflict(m)
            | AeroError::TooManyRequests(m) => m.clone(),
            other => other.to_string(),
        };
        let body = Json(json!({ "error": e.code(), "message": message, "status": status }));
        (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            body,
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, AeroApiError>;
