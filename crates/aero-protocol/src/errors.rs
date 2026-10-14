use thiserror::Error;

#[derive(Error, Debug)]
pub enum AeroError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Too many requests: {0}")]
    TooManyRequests(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Rate limited: credit balance exhausted")]
    RateLimited,
    #[error("Wasm execution error: {0}")]
    WasmExecution(String),
    #[error("Hazmat violation: {0}")]
    HazmatViolation(String),
    #[error("Remediation failed: {0}")]
    RemediationFailed(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("AI engine error: {0}")]
    AiEngine(String),
    #[error("Internal: {0}")]
    Internal(String),
}

impl AeroError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::TooManyRequests(_) => 429,
            Self::RateLimited => 429,
            Self::WasmExecution(_) | Self::HazmatViolation(_) | Self::RemediationFailed(_) => 422,
            Self::Database(_) | Self::AiEngine(_) => 503,
            Self::Internal(_) => 500,
        }
    }

    /// Short machine-readable code used in API error bodies.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::RateLimited => "credits_exhausted",
            Self::TooManyRequests(_) => "too_many_requests",
            Self::WasmExecution(_) => "wasm_execution",
            Self::HazmatViolation(_) => "hazmat_violation",
            Self::RemediationFailed(_) => "remediation_failed",
            Self::Database(_) => "database",
            Self::AiEngine(_) => "ai_engine",
            Self::Internal(_) => "internal",
        }
    }
}

pub type AeroResult<T> = Result<T, AeroError>;
