use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPack {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub framework: String,
    pub mcp_prompt_context: String,
    pub wasm_build_tool: WasmBuildTool,
    pub deployment_targets: Vec<DeploymentTarget>,
    pub security_constraints: SecurityConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmBuildTool {
    pub wasm_binary_url: String,
    pub wasm_hash: String,
    pub memory_limit_mb: u32,
    pub cpu_limit_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentTarget {
    LocalWasm,
    Vercel,
    Appwrite,
    Cloudflare,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConstraints {
    pub egress_filtering: bool,
    pub max_memory_mb: u32,
    pub max_cpu_percent: u32,
    pub timeout_ms: u32,
    pub hazmat_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployAutoDetectRequest {
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployAutoDetectResponse {
    pub detected_framework: String,
    pub skill_pack_id: String,
    pub build_duration_ms: u64,
    pub quarantine_passed: bool,
    pub remediation_applied: bool,
    pub deployed: bool,
    pub function_id: Option<uuid::Uuid>,
}
