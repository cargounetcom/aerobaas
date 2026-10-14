use std::time::Instant;

use aero_config::WasmConfig;
use aero_protocol::functions::{
    DeployRequest, DeployResponse, ExecuteResponse, Function, FunctionStatus, Runtime,
};
use aero_protocol::AeroResult;
use aero_remediation::RemediationService;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::hazmat::EgressFilter;
use crate::sandbox::SandboxConfig;

pub struct WasmRuntime {
    config: WasmConfig,
    #[allow(dead_code)] // wired up in a later phase (see ROADMAP.md)
    egress_filter: EgressFilter,
}

/// Largest module accepted by deploy (bytes).
pub const MAX_WASM_BYTES: usize = 10 * 1024 * 1024;

impl WasmRuntime {
    pub fn new(config: WasmConfig) -> anyhow::Result<Self> {
        let egress_filter = EgressFilter::new(
            config.allowed_egress_hosts.clone(),
            config.hazmat_egress_filtering,
        );
        Ok(Self {
            config,
            egress_filter,
        })
    }

    /// Check a module and build its record for `workspace_id`. Storing it is the caller's job.
    pub async fn deploy(
        &self,
        workspace_id: Uuid,
        req: &DeployRequest,
        _remediation: &RemediationService,
    ) -> AeroResult<(Function, DeployResponse)> {
        let start = Instant::now();
        let name = req.name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(aero_protocol::AeroError::BadRequest(
                "Function name must be 1–100 characters".into(),
            ));
        }
        if req.wasm_bytes.is_empty() || req.wasm_bytes.len() > MAX_WASM_BYTES {
            return Err(aero_protocol::AeroError::BadRequest(format!(
                "wasm_bytes must be 1 byte to {} MB",
                MAX_WASM_BYTES / 1024 / 1024
            )));
        }
        let entrypoint = req.entrypoint.trim();
        if entrypoint.is_empty() || entrypoint.len() > 200 {
            return Err(aero_protocol::AeroError::BadRequest(
                "entrypoint is required".into(),
            ));
        }

        let wasm_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&req.wasm_bytes);
            hex::encode(hasher.finalize())
        };
        let function_id = Uuid::new_v4();

        let quarantine_passed = if self.config.hazmat_egress_filtering {
            self.run_hazmat_quarantine(&req.wasm_bytes).await?
        } else {
            true
        };
        let status = if quarantine_passed {
            FunctionStatus::Deployed
        } else {
            FunctionStatus::Blocked
        };
        let clamp = |v: Option<u32>, max: u32| v.unwrap_or(max).clamp(1, max);
        let now = chrono::Utc::now();
        let function = Function {
            id: function_id,
            project_id: workspace_id,
            name: name.to_string(),
            runtime: Runtime::WasmWasi,
            wasm_hash: wasm_hash.clone(),
            entrypoint: entrypoint.to_string(),
            env_vars: vec![],
            memory_limit_mb: clamp(req.memory_limit_mb, self.config.max_memory_mb),
            cpu_limit_percent: clamp(req.cpu_limit_percent, self.config.max_cpu_percent),
            timeout_ms: clamp(req.timeout_ms, self.config.default_timeout_ms),
            status: status.clone(),
            created_at: now,
            updated_at: now,
        };
        tracing::info!(
            function_id = %function_id,
            deploy_duration_ms = start.elapsed().as_millis(),
            quarantine_passed,
            "Function checked"
        );
        let resp = DeployResponse {
            function_id,
            status,
            wasm_hash,
            quarantine_passed,
            remediation_applied: false,
        };
        Ok((function, resp))
    }

    pub async fn execute(
        &self,
        function: &Function,
        wasm_bytes: &[u8],
        payload: serde_json::Value,
    ) -> AeroResult<ExecuteResponse> {
        let start = Instant::now();
        if function.status != FunctionStatus::Deployed {
            return Err(aero_protocol::AeroError::WasmExecution(format!(
                "Function status is {:?}, not Deployed",
                function.status
            )));
        }
        let sandbox = SandboxConfig::new(
            function.memory_limit_mb,
            function.cpu_limit_percent,
            function.timeout_ms,
        );
        let result = self
            .run_sandboxed(function, wasm_bytes, &payload, &sandbox)
            .await?;
        let elapsed = start.elapsed();
        Ok(ExecuteResponse {
            execution_id: Uuid::new_v4(),
            output: result,
            duration_ms: elapsed.as_millis() as u64,
            memory_used_mb: 0.0,
            credits_consumed: 1,
        })
    }

    async fn run_hazmat_quarantine(&self, wasm_bytes: &[u8]) -> AeroResult<bool> {
        tracing::info!(
            "Running Hazmat quarantine scan on {} bytes",
            wasm_bytes.len()
        );

        let dangerous_patterns: &[&[u8]] = &[
            b"127.0.0.1",
            b"169.254.169.254",
            b"localhost",
            b"0.0.0.0",
            b"metadata.google",
            b"169.254",
        ];

        for pattern in dangerous_patterns {
            if contains_bytes(wasm_bytes, pattern) {
                tracing::warn!(pattern = ?pattern, "Hazmat violation: dangerous pattern detected");
                return Ok(false);
            }
        }

        tracing::info!("Hazmat quarantine passed");
        Ok(true)
    }

    async fn run_sandboxed(
        &self,
        function: &Function,
        _wasm_bytes: &[u8],
        payload: &serde_json::Value,
        sandbox: &SandboxConfig,
    ) -> AeroResult<serde_json::Value> {
        tracing::debug!(
            function_id = %function.id,
            memory_limit_mb = sandbox.memory_limit_mb,
            timeout_ms = sandbox.timeout_ms,
            "Executing in Wasm sandbox"
        );

        let _ = payload;

        Ok(serde_json::json!({
            "status": "executed",
            "message": "Wasm execution completed (stub — wire Wasmtime engine here)"
        }))
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}
