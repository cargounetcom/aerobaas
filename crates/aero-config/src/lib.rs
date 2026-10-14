use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AeroConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub wasm: WasmConfig,
    pub ai: AiConfig,
    pub security: SecurityConfig,
    pub orchestrator: OrchestratorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub namespace: String,
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
    pub encryption_key_env: String,
    pub hazmat_enabled: bool,
    pub remediation_enabled: bool,
    pub compliance_log_enabled: bool,
    pub max_severity_allowed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    pub erlang_node: String,
    pub erlang_cookie: String,
    pub max_agents: usize,
    pub agent_timeout_ms: u64,
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
            },
            database: DatabaseConfig {
                url: "surrealkv://data/aero.db".into(),
                namespace: "aerobaas".into(),
                max_connections: 50,
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
                encryption_key_env: "AERO_ENCRYPTION_KEY".into(),
                hazmat_enabled: true,
                remediation_enabled: true,
                compliance_log_enabled: true,
                max_severity_allowed: "medium".into(),
            },
            orchestrator: OrchestratorConfig {
                erlang_node: "aero_orchestrator@localhost".into(),
                erlang_cookie: "aero_default_cookie".into(),
                max_agents: 1_000_000,
                agent_timeout_ms: 300_000,
            },
        }
    }
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
        if let Some(v) = env("AERO_DB_URL") {
            c.database.url = v;
        }
        if let Some(v) = env("AERO_DB_NAMESPACE") {
            c.database.namespace = v;
        }
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
        assert!(c.validate().is_ok());
    }
}
