use std::collections::HashMap;
use std::time::Instant;

use aero_config::WasmConfig;
use aero_protocol::functions::{
    DeployRequest, DeployResponse, ExecuteResponse, Function, FunctionStatus, Runtime,
};
use aero_protocol::AeroResult;
use aero_remediation::RemediationService;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::hazmat::EgressFilter;
use crate::sandbox::SandboxConfig;

pub struct WasmRuntime {
    config: WasmConfig,
    instances: RwLock<HashMap<Uuid, WasmInstance>>,
    #[allow(dead_code)] // wired up in a later phase (see ROADMAP.md)
    egress_filter: EgressFilter,
}

#[derive(Clone)]
#[allow(dead_code)] // wired up in a later phase (see ROADMAP.md)
struct WasmInstance {
    function: Function,
    wasm_bytes: Vec<u8>,
    last_used: Instant,
}

impl WasmRuntime {
    pub fn new(config: WasmConfig) -> anyhow::Result<Self> {
        let egress_filter = EgressFilter::new(
            config.allowed_egress_hosts.clone(),
            config.hazmat_egress_filtering,
        );
        Ok(Self {
            config,
            instances: RwLock::new(HashMap::new()),
            egress_filter,
        })
    }

    pub async fn deploy(
        &self,
        req: DeployRequest,
        _remediation: &RemediationService,
    ) -> AeroResult<DeployResponse> {
        let start = Instant::now();

        let wasm_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&req.wasm_bytes);
            hex::encode(hasher.finalize())
        };

        let function_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        tracing::info!(
            function_id = %function_id,
            name = %req.name,
            wasm_size = req.wasm_bytes.len(),
            "Deploying Wasm function"
        );

        let quarantine_passed = if self.config.hazmat_egress_filtering {
            self.run_hazmat_quarantine(&req.wasm_bytes).await?
        } else {
            true
        };

        let remediation_applied = false;

        let status = if quarantine_passed {
            FunctionStatus::Deployed
        } else {
            FunctionStatus::Blocked
        };

        let function = Function {
            id: function_id,
            project_id,
            name: req.name.clone(),
            runtime: Runtime::WasmWasi,
            wasm_hash: wasm_hash.clone(),
            entrypoint: req.entrypoint,
            env_vars: vec![],
            memory_limit_mb: req.memory_limit_mb.unwrap_or(self.config.max_memory_mb),
            cpu_limit_percent: req.cpu_limit_percent.unwrap_or(self.config.max_cpu_percent),
            timeout_ms: req.timeout_ms.unwrap_or(self.config.default_timeout_ms),
            status: status.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.instances.write().insert(
            function_id,
            WasmInstance {
                function: function.clone(),
                wasm_bytes: req.wasm_bytes,
                last_used: Instant::now(),
            },
        );

        let elapsed = start.elapsed();
        tracing::info!(
            function_id = %function_id,
            deploy_duration_ms = elapsed.as_millis(),
            quarantine_passed,
            "Function deployed"
        );

        Ok(DeployResponse {
            function_id,
            status,
            wasm_hash,
            quarantine_passed,
            remediation_applied,
        })
    }

    pub async fn execute(
        &self,
        function_id: Uuid,
        payload: serde_json::Value,
    ) -> AeroResult<ExecuteResponse> {
        let start = Instant::now();

        let instance = {
            let instances = self.instances.read();
            instances.get(&function_id).cloned()
        };

        let Some(instance) = instance else {
            return Err(aero_protocol::AeroError::NotFound(format!(
                "Function {function_id} not found"
            )));
        };

        if instance.function.status != FunctionStatus::Deployed {
            return Err(aero_protocol::AeroError::WasmExecution(format!(
                "Function status is {:?}, not Deployed",
                instance.function.status
            )));
        }

        let sandbox = SandboxConfig::new(
            instance.function.memory_limit_mb,
            instance.function.cpu_limit_percent,
            instance.function.timeout_ms,
        );

        let result = self.run_sandboxed(&instance, &payload, &sandbox).await?;

        let elapsed = start.elapsed();
        let credits_consumed = ((elapsed.as_millis() as i64) + 1) / 10;

        Ok(ExecuteResponse {
            execution_id: Uuid::new_v4(),
            output: result,
            duration_ms: elapsed.as_millis() as u64,
            memory_used_mb: 0.0,
            credits_consumed,
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
        instance: &WasmInstance,
        payload: &serde_json::Value,
        sandbox: &SandboxConfig,
    ) -> AeroResult<serde_json::Value> {
        tracing::debug!(
            function_id = %instance.function.id,
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

    pub fn list_functions(&self) -> Vec<Function> {
        self.instances
            .read()
            .values()
            .map(|i| i.function.clone())
            .collect()
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
