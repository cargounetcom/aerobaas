use std::collections::HashMap;
use std::time::Instant;

use aero_protocol::metrics::*;
use parking_lot::RwLock;
use uuid::Uuid;

pub struct MetricsCollector {
    sandboxes: RwLock<HashMap<Uuid, SandboxMetrics>>,
    invocation_starts: RwLock<HashMap<Uuid, Instant>>,
    cold_starts: RwLock<Vec<f64>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            sandboxes: RwLock::new(HashMap::new()),
            invocation_starts: RwLock::new(HashMap::new()),
            cold_starts: RwLock::new(Vec::with_capacity(10_000)),
        }
    }

    pub fn register_sandbox(
        &self,
        function_id: Uuid,
        function_name: &str,
        memory_limit_mb: f64,
        cpu_limit_percent: f64,
    ) {
        let metrics = SandboxMetrics {
            function_id,
            function_name: function_name.to_string(),
            instance_id: Uuid::new_v4(),
            status: SandboxStatus::Idle,
            cold_start_ms: 0.0,
            execution_duration_ms: 0.0,
            memory_used_mb: 0.0,
            memory_limit_mb,
            cpu_percent: 0.0,
            cpu_limit_percent,
            network_egress_bytes: 0,
            network_ingress_bytes: 0,
            egress_blocked: 0,
            invocations_total: 0,
            errors_total: 0,
            last_invocation: None,
            timestamp: chrono::Utc::now(),
        };
        self.sandboxes.write().insert(function_id, metrics);
    }

    pub fn record_cold_start(&self, function_id: Uuid, duration_ms: f64) {
        self.cold_starts.write().push(duration_ms);
        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            s.cold_start_ms = duration_ms;
            s.status = SandboxStatus::Running;
            s.timestamp = chrono::Utc::now();
        }
    }

    pub fn start_invocation(&self, function_id: Uuid) {
        self.invocation_starts
            .write()
            .insert(function_id, Instant::now());
        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            s.status = SandboxStatus::Running;
            s.invocations_total += 1;
            s.last_invocation = Some(chrono::Utc::now());
            s.timestamp = chrono::Utc::now();
        }
    }

    pub fn end_invocation(
        &self,
        function_id: Uuid,
        success: bool,
        memory_used_mb: f64,
        cpu_percent: f64,
    ) {
        let duration = self
            .invocation_starts
            .write()
            .remove(&function_id)
            .map(|start| start.elapsed().as_millis() as f64)
            .unwrap_or(0.0);

        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            s.execution_duration_ms = duration;
            s.memory_used_mb = memory_used_mb;
            s.cpu_percent = cpu_percent;
            s.status = SandboxStatus::Idle;
            s.timestamp = chrono::Utc::now();
            if !success {
                s.errors_total += 1;
            }
        }
    }

    pub fn record_egress(&self, function_id: Uuid, bytes: u64, blocked: bool) {
        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            if blocked {
                s.egress_blocked += 1;
            } else {
                s.network_egress_bytes += bytes;
            }
            s.timestamp = chrono::Utc::now();
        }
    }

    pub fn record_ingress(&self, function_id: Uuid, bytes: u64) {
        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            s.network_ingress_bytes += bytes;
            s.timestamp = chrono::Utc::now();
        }
    }

    pub fn mark_blocked(&self, function_id: Uuid) {
        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            s.status = SandboxStatus::Blocked;
            s.errors_total += 1;
            s.timestamp = chrono::Utc::now();
        }
    }

    pub fn mark_quarantine(&self, function_id: Uuid) {
        let mut sandboxes = self.sandboxes.write();
        if let Some(s) = sandboxes.get_mut(&function_id) {
            s.status = SandboxStatus::Quarantine;
            s.timestamp = chrono::Utc::now();
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let sandboxes: Vec<SandboxMetrics> = self.sandboxes.read().values().cloned().collect();
        let aggregate = self.aggregate(&sandboxes);
        MetricsSnapshot {
            timestamp: chrono::Utc::now(),
            sandboxes,
            aggregate,
        }
    }

    fn aggregate(&self, sandboxes: &[SandboxMetrics]) -> AggregateMetrics {
        let total = sandboxes.len() as u32;
        let active = sandboxes
            .iter()
            .filter(|s| s.status == SandboxStatus::Running)
            .count() as u32;
        let idle = sandboxes
            .iter()
            .filter(|s| s.status == SandboxStatus::Idle)
            .count() as u32;
        let blocked = sandboxes
            .iter()
            .filter(|s| s.status == SandboxStatus::Blocked)
            .count() as u32;

        let total_invocations: u64 = sandboxes.iter().map(|s| s.invocations_total).sum();
        let total_errors: u64 = sandboxes.iter().map(|s| s.errors_total).sum();

        let mut cold_starts: Vec<f64> = self.cold_starts.read().clone();
        cold_starts.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg_cold = if cold_starts.is_empty() {
            0.0
        } else {
            cold_starts.iter().sum::<f64>() / cold_starts.len() as f64
        };
        let p50 = percentile(&cold_starts, 0.50);
        let p95 = percentile(&cold_starts, 0.95);
        let p99 = percentile(&cold_starts, 0.99);

        let avg_mem = if total > 0 {
            sandboxes.iter().map(|s| s.memory_used_mb).sum::<f64>() / total as f64
        } else {
            0.0
        };
        let max_mem = sandboxes
            .iter()
            .map(|s| s.memory_used_mb)
            .fold(0.0, f64::max);
        let avg_cpu = if total > 0 {
            sandboxes.iter().map(|s| s.cpu_percent).sum::<f64>() / total as f64
        } else {
            0.0
        };

        let total_egress: u64 = sandboxes.iter().map(|s| s.network_egress_bytes).sum();
        let total_egress_blocked: u32 = sandboxes.iter().map(|s| s.egress_blocked).sum();

        AggregateMetrics {
            total_sandboxes: total,
            active_sandboxes: active,
            idle_sandboxes: idle,
            blocked_sandboxes: blocked,
            total_invocations,
            total_errors,
            avg_cold_start_ms: avg_cold,
            p50_cold_start_ms: p50,
            p95_cold_start_ms: p95,
            p99_cold_start_ms: p99,
            avg_memory_mb: avg_mem,
            max_memory_mb: max_mem,
            avg_cpu_percent: avg_cpu,
            total_egress_bytes: total_egress,
            total_egress_blocked,
            rps: 0.0,
        }
    }

    pub fn get_sandbox(&self, function_id: Uuid) -> Option<SandboxMetrics> {
        self.sandboxes.read().get(&function_id).cloned()
    }

    pub fn list_sandboxes(&self) -> Vec<SandboxMetrics> {
        self.sandboxes.read().values().cloned().collect()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p * sorted.len() as f64) - 1.0).max(0.0) as usize;
    sorted[idx.min(sorted.len() - 1)]
}
