use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeroConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub wasm: WasmConfig,
    pub ai: AiConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    /// Browser origins allowed to call the API (`AERO_CORS_ORIGINS`, comma separated). Empty = same origin only.
    pub cors_origins: Vec<String>,
    /// Trust `X-Forwarded-For` / `X-Real-IP` (`AERO_TRUST_PROXY=true`) — only behind your own reverse proxy.
    pub trust_proxy: bool,
    /// Public URL of the admin, used in invite links (`AERO_PUBLIC_URL`).
    pub public_url: String,
    /// How long request-inspector rows are kept (`AERO_INSPECTOR_RETENTION_DAYS`).
    pub inspector_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL URL (`AERO_DATABASE_URL`). Contains a password: never logged or serialised.
    #[serde(skip_serializing, default)]
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    pub max_memory_mb: u32,
    pub max_cpu_percent: u32,
    pub default_timeout_ms: u32,
    pub max_instances: usize,
    pub hazmat_egress_filtering: bool,
    pub allowed_egress_hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub model_name: String,
    pub model_path: Option<String>,
    pub quantization: String,
    pub max_context_length: usize,
    pub gpu_backend: GpuBackend,
    pub idle_memory_limit_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuBackend {
    Cpu,
    Cuda,
    Metal,
    Wgpu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// HMAC secret for JWTs (`AERO_JWT_SECRET`, at least 32 characters). Never logged or serialised.
    #[serde(skip_serializing, default)]
    pub jwt_secret: String,
    /// HMAC key for signed compliance records (`AERO_SIGNING_KEY`). Derived from the JWT secret when unset.
    #[serde(skip_serializing, default)]
    pub signing_key: String,
    /// Emails allowed to manage platform-wide settings (`AERO_PLATFORM_ADMINS`, comma separated).
    /// Empty = the first registered user.
    pub platform_admins: Vec<String>,
    /// Max failed logins per email in 15 minutes (`AERO_LOGIN_MAX_FAILURES`).
    pub login_max_failures: u32,
    pub hazmat_enabled: bool,
    pub remediation_enabled: bool,
    pub compliance_log_enabled: bool,
    pub max_severity_allowed: String,
}

impl Default for AeroConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".into(),
                port: 8080,
                max_connections: 100_000,
                tls_enabled: false,
                tls_cert_path: None,
                tls_key_path: None,
                cors_origins: vec![],
                trust_proxy: false,
                public_url: "http://localhost:8080".into(),
                inspector_retention_days: 30,
            },
            database: DatabaseConfig {
                url: String::new(),
                max_connections: 20,
                connection_timeout_ms: 5000,
            },
            wasm: WasmConfig {
                max_memory_mb: 128,
                max_cpu_percent: 100,
                default_timeout_ms: 30_000,
                max_instances: 1000,
                hazmat_egress_filtering: true,
                allowed_egress_hosts: vec![],
            },
            ai: AiConfig {
                model_name: "Qwen-2.5-Coder-1.5B-Instruct".into(),
                model_path: None,
                quantization: "Q4".into(),
                max_context_length: 4096,
                gpu_backend: GpuBackend::Cpu,
                idle_memory_limit_mb: 2048,
            },
            security: SecurityConfig {
                jwt_secret: String::new(),
                signing_key: String::new(),
                platform_admins: vec![],
                login_max_failures: 10,
                hazmat_enabled: true,
                remediation_enabled: true,
                compliance_log_enabled: true,
                max_severity_allowed: "medium".into(),
            },
        }
    }
}

fn list(v: &str) -> Vec<String> {
    v.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

impl AeroConfig {
    /// Defaults overridden by the `AERO_*` environment variables documented in `.env.example`.
    pub fn from_env() -> Self {
        let mut c = Self::default();
        if let Some(v) = env("AERO_SERVER_HOST") {
            c.server.host = v;
        }
        if let Some(v) = env("AERO_SERVER_PORT").and_then(|v| v.parse().ok()) {
            c.server.port = v;
        }
        if let Some(v) = env("AERO_DATABASE_URL").or_else(|| env("DATABASE_URL")) {
            c.database.url = v;
        }
        if let Some(v) = env("AERO_DATABASE_MAX_CONNECTIONS").and_then(|v| v.parse().ok()) {
            c.database.max_connections = v;
        }
        if let Some(v) = env("AERO_CORS_ORIGINS") {
            c.server.cors_origins = list(&v);
        }
        if let Some(v) = env("AERO_TRUST_PROXY") {
            c.server.trust_proxy = v == "true";
        }
        if let Some(v) = env("AERO_PUBLIC_URL") {
            c.server.public_url = v.trim_end_matches('/').to_string();
        }
        if let Some(v) = env("AERO_INSPECTOR_RETENTION_DAYS").and_then(|v| v.parse().ok()) {
            c.server.inspector_retention_days = v;
        }
        if let Some(v) = env("AERO_PLATFORM_ADMINS") {
            c.security.platform_admins = list(&v).into_iter().map(|e| e.to_lowercase()).collect();
        }
        if let Some(v) = env("AERO_LOGIN_MAX_FAILURES").and_then(|v| v.parse().ok()) {
            c.security.login_max_failures = v;
        }
        c.security.signing_key = env("AERO_SIGNING_KEY").unwrap_or_default();
        if let Some(v) = env("AERO_WASM_MAX_MEMORY_MB").and_then(|v| v.parse().ok()) {
            c.wasm.max_memory_mb = v;
        }
        if let Some(v) = env("AERO_WASM_HAZMAT_EGRESS") {
            c.wasm.hazmat_egress_filtering = v == "true";
        }
        if let Some(v) = env("AERO_AI_MODEL") {
            c.ai.model_name = v;
        }
        if let Some(v) = env("AERO_AI_QUANTIZATION") {
            c.ai.quantization = v;
        }
        if let Some(v) = env("AERO_HAZMAT_ENABLED") {
            c.security.hazmat_enabled = v == "true";
        }
        if let Some(v) = env("AERO_REMEDIATION_ENABLED") {
            c.security.remediation_enabled = v == "true";
        }
        c.security.jwt_secret = env("AERO_JWT_SECRET").unwrap_or_default();
        c
    }

    /// Refuse to start with missing or weak secrets.
    pub fn validate(&self) -> Result<(), String> {
        if self.security.jwt_secret.len() < 32 {
            return Err("AERO_JWT_SECRET must be set and at least 32 characters (generate: openssl rand -base64 48)".into());
        }
        if self.security.jwt_secret.contains("change-me") {
            return Err(
                "AERO_JWT_SECRET still has the example value; generate a real secret".into(),
            );
        }
        let url = &self.database.url;
        if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
            return Err(
                "AERO_DATABASE_URL must be set to a PostgreSQL URL (postgres://user:pass@host:5432/db)"
                    .into(),
            );
        }
        if !self.security.signing_key.is_empty() && self.security.signing_key.len() < 32 {
            return Err("AERO_SIGNING_KEY must be at least 32 characters".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_jwt_secret_is_rejected() {
        let mut c = AeroConfig::default();
        assert!(c.validate().is_err());
        c.security.jwt_secret = "change-me-in-production-change-me-123".into();
        assert!(c.validate().is_err());
        c.security.jwt_secret = "x".repeat(48);
        assert!(c.validate().is_err(), "database URL is required");
        c.database.url = "postgres://aero:aero@localhost/aero".into();
        assert!(c.validate().is_ok());
    }
}
