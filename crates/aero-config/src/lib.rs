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
