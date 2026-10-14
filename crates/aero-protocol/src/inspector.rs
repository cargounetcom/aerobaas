use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTrace {
    pub id: Uuid,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub duration_ms: f64,
    pub request_headers: HashMap<String, String>,
    pub response_headers: HashMap<String, String>,
    pub request_body_size: usize,
    pub response_body_size: usize,
    pub user_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorConfig {
    pub enabled: bool,
    pub max_traces: usize,
    pub sample_rate: f64,
    pub log_bodies: bool,
    pub slow_request_threshold_ms: f64,
}

impl Default for InspectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_traces: 10_000,
            sample_rate: 1.0,
            log_bodies: false,
            slow_request_threshold_ms: 500.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorStats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub rps: f64,
    pub active_connections: u64,
    pub traces: Vec<RequestTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorQuery {
    pub method: Option<String>,
    pub path_prefix: Option<String>,
    pub min_status: Option<u16>,
    pub max_status: Option<u16>,
    pub min_duration_ms: Option<f64>,
    pub user_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub limit: Option<usize>,
}
