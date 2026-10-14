use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use aero_protocol::AeroError;

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
        let (status, message) = match self {
            Self::Aero(e) => (e.status_code(), e.to_string()),
            Self::BadRequest(msg) => (400, msg),
        };
        let body = Json(json!({ "error": message, "code": status }));
        (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), body).into_response()
    }
}
