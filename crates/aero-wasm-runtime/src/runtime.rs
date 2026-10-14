//! Wasm function runtime on Wasmtime.
//!
//! Function contract (WASI preview 1 "command" module):
//! * the request payload (JSON) arrives on **stdin**;
//! * the function writes its result to **stdout** (JSON, or plain text);
//! * **stderr** is returned as logs;
//! * the entrypoint is an exported `() -> ()` function, `_start` by default; exit code 0 = success.
//!
//! Sandbox: only `wasi_snapshot_preview1` imports are allowed; no files, no environment, no
//! network (WASI p1 cannot open sockets); memory, CPU (fuel) and wall-clock time are limited.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aero_config::WasmConfig;
use aero_protocol::functions::{
    DeployRequest, DeployResponse, ExecuteResponse, Function, FunctionStatus, Runtime,
};
use aero_protocol::{AeroError, AeroResult};
use aero_remediation::RemediationService;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use wasmtime::{
    Config, Engine, ExternType, InstancePre, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder, Trap,
};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::{I32Exit, WasiCtxBuilder};

use crate::hazmat::EgressFilter;
use crate::sandbox::SandboxConfig;

/// Largest module accepted by deploy (bytes).
pub const MAX_WASM_BYTES: usize = 10 * 1024 * 1024;
/// Largest stdout a function may produce.
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_LOG_BYTES: usize = 64 * 1024;
/// Epoch tick used for wall-clock timeouts.
const TICK: Duration = Duration::from_millis(10);
/// CPU budget: fuel units (≈ wasm instructions) per millisecond of allowed time.
const FUEL_PER_MS: u64 = 2_000_000;
const CACHE_SIZE: usize = 256;
const WASI_MODULE: &str = "wasi_snapshot_preview1";

struct State {
    wasi: WasiP1Ctx,
    limits: StoreLimits,
}

struct Cached {
    pre: InstancePre<State>,
    last_used: Instant,
}

pub struct WasmRuntime {
    config: WasmConfig,
    engine: Engine,
    linker: Linker<State>,
    cache: Mutex<HashMap<String, Cached>>,
    #[allow(dead_code)] // host HTTP with an allow-list comes later (see ROADMAP.md)
    egress_filter: EgressFilter,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    Ok,
    Error,
    Timeout,
    OutOfFuel,
    OutOfMemory,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Timeout => "timeout",
            Self::OutOfFuel => "out_of_fuel",
            Self::OutOfMemory => "out_of_memory",
        }
    }
}

/// Result of one execution (also for failed runs, so they can be recorded).
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub status: RunStatus,
    pub output: serde_json::Value,
    pub logs: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub fuel_used: u64,
    pub memory_mb: f64,
    pub cold_start: bool,
}

impl RunOutcome {
    pub fn into_response(self, execution_id: Uuid) -> ExecuteResponse {
        ExecuteResponse {
            execution_id,
            output: self.output,
            duration_ms: self.duration_ms,
            memory_used_mb: self.memory_mb,
            credits_consumed: 1,
            status: self.status.as_str().to_string(),
            logs: self.logs,
            fuel_used: self.fuel_used,
            cold_start: self.cold_start,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn bad(msg: impl Into<String>) -> AeroError {
    AeroError::BadRequest(msg.into())
}

impl WasmRuntime {
    pub fn new(config: WasmConfig) -> anyhow::Result<Self> {
        let mut cfg = Config::new();
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        let engine = Engine::new(&cfg).map_err(|e| anyhow::anyhow!("wasmtime engine: {e}"))?;
        let mut linker: Linker<State> = Linker::new(&engine);
        p1::add_to_linker_sync(&mut linker, |s: &mut State| &mut s.wasi)
            .map_err(|e| anyhow::anyhow!("wasi linker: {e}"))?;

        // Wall-clock timeouts: advance the engine epoch every TICK.
        let ticker = engine.weak();
        std::thread::Builder::new()
            .name("aero-wasm-epoch".into())
            .spawn(move || loop {
                std::thread::sleep(TICK);
                match ticker.upgrade() {
                    Some(e) => e.increment_epoch(),
                    None => break,
                }
            })?;

        let egress_filter = EgressFilter::new(
            config.allowed_egress_hosts.clone(),
            config.hazmat_egress_filtering,
        );
        Ok(Self {
            config,
            engine,
            linker,
            cache: Mutex::new(HashMap::new()),
            egress_filter,
        })
    }

    /// Compile and check a module: valid WebAssembly, only WASI preview 1 imports, and an
    /// exported `() -> ()` entrypoint. Returns the ready-to-instantiate module.
    fn prepare(&self, wasm: &[u8], entrypoint: &str) -> AeroResult<InstancePre<State>> {
        let module = Module::new(&self.engine, wasm)
            .map_err(|e| bad(format!("Not a valid WebAssembly module: {e}")))?;
        for import in module.imports() {
            if import.module() != WASI_MODULE {
                return Err(bad(format!(
                    "The module imports `{}.{}`; only WASI preview 1 (`{WASI_MODULE}`) is available",
                    import.module(),
                    import.name()
                )));
            }
        }
        match module.get_export(entrypoint) {
            Some(ExternType::Func(f)) if f.params().len() == 0 && f.results().len() == 0 => {}
            Some(_) => {
                return Err(bad(format!(
                    "Export `{entrypoint}` must be a function without parameters or results"
                )))
            }
            None => {
                return Err(bad(format!(
                    "The module has no exported function `{entrypoint}` (build a WASI command, e.g. --target wasm32-wasip1)"
                )))
            }
        }
        self.linker
            .instantiate_pre(&module)
            .map_err(|e| bad(format!("The module cannot be linked: {e}")))
    }

    fn cache_put(&self, hash: &str, pre: InstancePre<State>) {
        let mut c = self.cache.lock();
        if c.len() >= CACHE_SIZE {
            if let Some(oldest) = c
                .iter()
                .min_by_key(|(_, v)| v.last_used)
                .map(|(k, _)| k.clone())
            {
                c.remove(&oldest);
            }
        }
        c.insert(
            hash.to_string(),
            Cached {
                pre,
                last_used: Instant::now(),
            },
        );
    }

    fn cache_get(&self, hash: &str) -> Option<InstancePre<State>> {
        let mut c = self.cache.lock();
        c.get_mut(hash).map(|v| {
            v.last_used = Instant::now();
            v.pre.clone()
        })
    }

    /// Validate a module and build its record for `workspace_id`. Storing it is the caller's job.
    pub async fn deploy(
        self: &Arc<Self>,
        workspace_id: Uuid,
        req: &DeployRequest,
        wasm: &[u8],
        _remediation: &RemediationService,
    ) -> AeroResult<(Function, DeployResponse)> {
        let start = Instant::now();
        let name = req.name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(bad("Function name must be 1–100 characters"));
        }
        if wasm.is_empty() || wasm.len() > MAX_WASM_BYTES {
            return Err(bad(format!(
                "The module must be 1 byte to {} MB",
                MAX_WASM_BYTES / 1024 / 1024
            )));
        }
        let entrypoint = match req.entrypoint.trim() {
            "" | "main" => "_start".to_string(),
            e if e.len() <= 200 => e.to_string(),
            _ => return Err(bad("entrypoint is too long")),
        };
        let wasm_hash = hex(&Sha256::digest(wasm));

        // Compile off the async runtime; keep the result warm in the cache.
        let rt = self.clone();
        let bytes = wasm.to_vec();
        let ep = entrypoint.clone();
        let pre = tokio::task::spawn_blocking(move || rt.prepare(&bytes, &ep))
            .await
            .map_err(|e| AeroError::Internal(format!("compile task: {e}")))??;
        self.cache_put(&format!("{wasm_hash}:{entrypoint}"), pre);

        let clamp = |v: Option<u32>, max: u32| v.unwrap_or(max).clamp(1, max);
        let now = chrono::Utc::now();
        let function = Function {
            id: Uuid::new_v4(),
            project_id: workspace_id,
            name: name.to_string(),
            runtime: Runtime::WasmWasi,
            wasm_hash: wasm_hash.clone(),
            entrypoint,
            env_vars: vec![],
            memory_limit_mb: clamp(req.memory_limit_mb, self.config.max_memory_mb),
            cpu_limit_percent: clamp(req.cpu_limit_percent, self.config.max_cpu_percent),
            timeout_ms: clamp(req.timeout_ms, self.config.default_timeout_ms),
            status: FunctionStatus::Deployed,
            created_at: now,
            updated_at: now,
        };
        tracing::info!(
            function_id = %function.id,
            compile_ms = start.elapsed().as_millis() as u64,
            "Function validated and compiled"
        );
        let resp = DeployResponse {
            function_id: function.id,
            status: FunctionStatus::Deployed,
            wasm_hash,
            quarantine_passed: true,
            remediation_applied: false,
        };
        Ok((function, resp))
    }

    /// Run a function. Sandbox failures (timeout, fuel, memory, traps, non-zero exit) come back
    /// as an outcome with a non-ok status, not as an error.
    pub async fn execute(
        self: &Arc<Self>,
        function: &Function,
        wasm: Vec<u8>,
        payload: serde_json::Value,
    ) -> AeroResult<RunOutcome> {
        if function.status != FunctionStatus::Deployed {
            return Err(AeroError::WasmExecution(format!(
                "Function status is {:?}, not Deployed",
                function.status
            )));
        }
        let sandbox = SandboxConfig::new(
            function.memory_limit_mb,
            function.cpu_limit_percent,
            function.timeout_ms,
        );
        let input = serde_json::to_vec(&payload).unwrap_or_default();
        let rt = self.clone();
        let f = function.clone();
        tokio::task::spawn_blocking(move || rt.run_blocking(&f, &wasm, input, &sandbox))
            .await
            .map_err(|e| AeroError::Internal(format!("execution task: {e}")))?
    }

    fn run_blocking(
        &self,
        function: &Function,
        wasm: &[u8],
        input: Vec<u8>,
        sandbox: &SandboxConfig,
    ) -> AeroResult<RunOutcome> {
        let start = Instant::now();
        let key = format!("{}:{}", function.wasm_hash, function.entrypoint);
        let (pre, cold_start) = match self.cache_get(&key) {
            Some(p) => (p, false),
            None => {
                let p = self.prepare(wasm, &function.entrypoint)?;
                self.cache_put(&key, p.clone());
                (p, true)
            }
        };

        let stdout = MemoryOutputPipe::new(MAX_OUTPUT_BYTES);
        let stderr = MemoryOutputPipe::new(MAX_LOG_BYTES);
        let wasi = WasiCtxBuilder::new()
            .stdin(MemoryInputPipe::new(input))
            .stdout(stdout.clone())
            .stderr(stderr.clone())
            .args(&[function.name.as_str()])
            .allow_tcp(false)
            .allow_udp(false)
            .allow_ip_name_lookup(false)
            .build_p1();
        let limits = StoreLimitsBuilder::new()
            .memory_size(sandbox.memory_limit_bytes() as usize)
            .instances(1)
            .memories(1)
            .tables(16)
            .table_elements(100_000)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&self.engine, State { wasi, limits });
        store.limiter(|s| &mut s.limits);
        let fuel = (sandbox.timeout_ms as u64).max(1) * FUEL_PER_MS;
        store
            .set_fuel(fuel)
            .map_err(|e| AeroError::Internal(format!("fuel: {e}")))?;
        let ticks = (sandbox.timeout_ms as u64 / TICK.as_millis() as u64).max(1) + 1;
        store.set_epoch_deadline(ticks);
        store.epoch_deadline_trap();

        let result = pre.instantiate(&mut store).and_then(|instance| {
            let memory = instance.get_memory(&mut store, "memory");
            let run = instance
                .get_typed_func::<(), ()>(&mut store, &function.entrypoint)?
                .call(&mut store, ());
            let mem = memory.map(|m| m.data_size(&store)).unwrap_or(0);
            run.map(|_| mem)
        });

        let fuel_used = fuel.saturating_sub(store.get_fuel().unwrap_or(0));
        let logs = String::from_utf8_lossy(&stderr.contents()).into_owned();
        let out = stdout.contents();
        let mut outcome = RunOutcome {
            status: RunStatus::Ok,
            output: serde_json::Value::Null,
            logs,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            fuel_used,
            memory_mb: 0.0,
            cold_start,
        };
        let finish_output = |o: &mut RunOutcome| {
            if !out.is_empty() {
                o.output = serde_json::from_slice(&out).unwrap_or_else(|_| {
                    serde_json::Value::String(String::from_utf8_lossy(&out).into_owned())
                });
            }
        };
        match result {
            Ok(mem) => {
                outcome.memory_mb = mem as f64 / 1024.0 / 1024.0;
                finish_output(&mut outcome);
            }
            Err(e) => {
                if let Some(exit) = e.downcast_ref::<I32Exit>() {
                    finish_output(&mut outcome);
                    if exit.0 != 0 {
                        outcome.status = RunStatus::Error;
                        outcome.error = Some(format!("exited with code {}", exit.0));
                    }
                } else {
                    let (status, msg) = match e.downcast_ref::<Trap>() {
                        Some(Trap::Interrupt) => (
                            RunStatus::Timeout,
                            format!("timed out after {} ms", sandbox.timeout_ms),
                        ),
                        Some(Trap::OutOfFuel) => {
                            (RunStatus::OutOfFuel, "CPU budget exhausted".to_string())
                        }
                        _ => {
                            let text = format!("{e:#}");
                            if text.contains("memory") && text.contains("grow")
                                || text.contains("resource limit")
                            {
                                (
                                    RunStatus::OutOfMemory,
                                    format!(
                                        "memory limit of {} MB reached",
                                        sandbox.memory_limit_mb
                                    ),
                                )
                            } else {
                                (RunStatus::Error, text)
                            }
                        }
                    };
                    outcome.status = status;
                    outcome.error = Some(msg);
                    finish_output(&mut outcome);
                }
            }
        }
        outcome.duration_ms = start.elapsed().as_millis() as u64;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> Arc<WasmRuntime> {
        let mut c = aero_config::AeroConfig::default().wasm;
        c.default_timeout_ms = 500;
        c.max_memory_mb = 16;
        Arc::new(WasmRuntime::new(c).unwrap())
    }

    fn func(wasm: &[u8]) -> Function {
        Function {
            id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            name: "t".into(),
            runtime: Runtime::WasmWasi,
            wasm_hash: hex(&Sha256::digest(wasm)),
            entrypoint: "_start".into(),
            env_vars: vec![],
            memory_limit_mb: 16,
            cpu_limit_percent: 100,
            timeout_ms: 500,
            status: FunctionStatus::Deployed,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    pub const ECHO: &str = r#"(module
      (import "wasi_snapshot_preview1" "fd_read" (func $read (param i32 i32 i32 i32) (result i32)))
      (import "wasi_snapshot_preview1" "fd_write" (func $write (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (func (export "_start")
        (i32.store (i32.const 0) (i32.const 64))
        (i32.store (i32.const 4) (i32.const 4096))
        (drop (call $read (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 8)))
        (i32.store (i32.const 16) (i32.const 64))
        (i32.store (i32.const 20) (i32.load (i32.const 8)))
        (drop (call $write (i32.const 1) (i32.const 16) (i32.const 1) (i32.const 24)))))"#;

    #[tokio::test]
    async fn echo_roundtrip_and_cache() {
        let rt = rt();
        let wasm = wat::parse_str(ECHO).unwrap();
        let f = func(&wasm);
        let o = rt
            .execute(&f, wasm.clone(), serde_json::json!({"hello": "world"}))
            .await
            .unwrap();
        assert_eq!(o.status, RunStatus::Ok, "{o:?}");
        assert_eq!(o.output, serde_json::json!({"hello": "world"}));
        assert!(o.cold_start);
        let o2 = rt.execute(&f, wasm, serde_json::json!(1)).await.unwrap();
        assert!(!o2.cold_start);
        assert_eq!(o2.output, serde_json::json!(1));
    }

    #[tokio::test]
    async fn infinite_loop_is_stopped() {
        let rt = rt();
        let wasm = wat::parse_str(r#"(module (func (export "_start") (loop (br 0))))"#).unwrap();
        let o = rt
            .execute(&func(&wasm), wasm, serde_json::Value::Null)
            .await
            .unwrap();
        assert!(
            matches!(o.status, RunStatus::Timeout | RunStatus::OutOfFuel),
            "{o:?}"
        );
        assert!(o.duration_ms < 3000, "{o:?}");
    }

    #[tokio::test]
    async fn memory_limit_is_enforced() {
        let rt = rt();
        // grows to 32 MB (512 pages) with a 16 MB limit
        let wasm = wat::parse_str(
            r#"(module (memory (export "memory") 1)
                 (func (export "_start") (drop (memory.grow (i32.const 511)))))"#,
        )
        .unwrap();
        let o = rt
            .execute(&func(&wasm), wasm, serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(o.status, RunStatus::OutOfMemory, "{o:?}");
    }

    #[test]
    fn imports_and_entrypoint_are_checked() {
        let rt = rt();
        assert!(rt.prepare(&[1, 2, 3], "_start").is_err());
        let env =
            wat::parse_str(r#"(module (import "env" "socket" (func)) (func (export "_start")))"#)
                .unwrap();
        let err = rt.prepare(&env, "_start").err().unwrap().to_string();
        assert!(err.contains("env.socket"), "{err}");
        let none = wat::parse_str(r#"(module (func (export "run")))"#).unwrap();
        assert!(rt.prepare(&none, "_start").is_err());
        assert!(rt.prepare(&none, "run").is_ok());
    }

    /// Prints cold/warm start timings. Real numbers: `cargo test --release -p aero-wasm-runtime timings -- --nocapture`
    #[tokio::test]
    async fn timings() {
        let rt = rt();
        let wasm = wat::parse_str(ECHO).unwrap();
        let f = func(&wasm);
        let cold = rt
            .execute(&f, wasm.clone(), serde_json::json!({}))
            .await
            .unwrap();
        assert!(cold.cold_start);
        let mut warm: Vec<u64> = Vec::new();
        for _ in 0..50 {
            let t = Instant::now();
            let o = rt
                .execute(&f, wasm.clone(), serde_json::json!({"n": 1}))
                .await
                .unwrap();
            assert_eq!(o.status, RunStatus::Ok);
            warm.push(t.elapsed().as_micros() as u64);
        }
        warm.sort_unstable();
        let p = |q: f64| warm[((warm.len() - 1) as f64 * q) as usize];
        eprintln!(
            "wasm timings: cold {} ms (compile + run); warm p50 {} µs, p95 {} µs",
            cold.duration_ms,
            p(0.5),
            p(0.95)
        );
        assert!(p(0.95) < 500_000, "warm p95 should be well under 500 ms");
    }
}
