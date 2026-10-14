use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxMetrics {
    pub function_id: Uuid,
    pub function_name: String,
    pub instance_id: Uuid,
    pub status: SandboxStatus,
    pub cold_start_ms: f64,
    pub execution_duration_ms: f64,
    pub memory_used_mb: f64,
    pub memory_limit_mb: f64,
    pub cpu_percent: f64,
    pub cpu_limit_percent: f64,
    pub network_egress_bytes: u64,
    pub network_ingress_bytes: u64,
    pub egress_blocked: u32,
    pub invocations_total: u64,
    pub errors_total: u64,
    pub last_invocation: Option<chrono::DateTime<chrono::Utc>>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    Idle,
    Running,
    ColdStart,
    Quarantine,
    Blocked,
    Crashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub sandboxes: Vec<SandboxMetrics>,
    pub aggregate: AggregateMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateMetrics {
    pub total_sandboxes: u32,
    pub active_sandboxes: u32,
    pub idle_sandboxes: u32,
    pub blocked_sandboxes: u32,
    pub total_invocations: u64,
    pub total_errors: u64,
    pub avg_cold_start_ms: f64,
    pub p50_cold_start_ms: f64,
    pub p95_cold_start_ms: f64,
    pub p99_cold_start_ms: f64,
    pub avg_memory_mb: f64,
    pub max_memory_mb: f64,
    pub avg_cpu_percent: f64,
    pub total_egress_bytes: u64,
    pub total_egress_blocked: u32,
    pub rps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdStartHistoryPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub function_id: Uuid,
    pub function_name: String,
    pub cold_start_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryThrottlingPoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub function_id: Uuid,
    pub function_name: String,
    pub memory_used_mb: f64,
    pub memory_limit_mb: f64,
    pub throttled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsHistory {
    pub cold_start_history: Vec<ColdStartHistoryPoint>,
    pub memory_history: Vec<MemoryThrottlingPoint>,
    pub window_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQuery {
    pub workspace_id: Option<Uuid>,
    pub function_id: Option<Uuid>,
    pub status: Option<SandboxStatus>,
    pub window_minutes: Option<u32>,
}
