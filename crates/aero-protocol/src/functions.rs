use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub runtime: Runtime,
    pub wasm_hash: String,
    pub entrypoint: String,
    pub env_vars: Vec<EnvVar>,
    pub memory_limit_mb: u32,
    pub cpu_limit_percent: u32,
    pub timeout_ms: u32,
    pub status: FunctionStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    WasmWasi,
    WasmEmscripten,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub encrypted_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FunctionStatus {
    Draft,
    InQuarantine,
    Deployed,
    Blocked,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRequest {
    pub name: String,
    pub wasm_bytes: Vec<u8>,
    pub entrypoint: String,
    pub env_vars: Option<Vec<EnvVarInput>>,
    pub memory_limit_mb: Option<u32>,
    pub cpu_limit_percent: Option<u32>,
    pub timeout_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarInput {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResponse {
    pub function_id: Uuid,
    pub status: FunctionStatus,
    pub wasm_hash: String,
    pub quarantine_passed: bool,
    pub remediation_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// Optional; the function id in the URL is authoritative.
    #[serde(default)]
    pub function_id: Option<Uuid>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub execution_id: Uuid,
    pub output: serde_json::Value,
    pub duration_ms: u64,
    pub memory_used_mb: f64,
    pub credits_consumed: i64,
}
