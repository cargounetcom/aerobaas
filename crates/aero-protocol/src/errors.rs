use thiserror::Error;

#[derive(Error, Debug)]
pub enum AeroError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: scope `{0}` not granted")]
    Forbidden(String),
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
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::RateLimited => 429,
            Self::WasmExecution(_) | Self::HazmatViolation(_) | Self::RemediationFailed(_) => 422,
            Self::Database(_) | Self::AiEngine(_) => 503,
            Self::Internal(_) => 500,
        }
    }
}

pub type AeroResult<T> = Result<T, AeroError>;
