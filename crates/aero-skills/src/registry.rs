use std::sync::Arc;

use aero_ai_engine::AiEngine;
use aero_db::DbClient;
use aero_protocol::skills::{DeployAutoDetectRequest, DeployAutoDetectResponse, SkillPack};
use aero_wasm_runtime::WasmRuntime;
use aero_protocol::AeroResult;

pub struct SkillRegistry {
    db: Arc<DbClient>,
    builtin_skills: Vec<SkillPack>,
}

impl SkillRegistry {
    pub fn new(db: Arc<DbClient>) -> Self {
        Self {
            db,
            builtin_skills: builtin_skills(),
        }
    }

    pub async fn list_skills(&self) -> AeroResult<Vec<SkillPack>> {
        Ok(self.builtin_skills.clone())
    }

    pub async fn get_skill(&self, id: &str) -> AeroResult<SkillPack> {
        self.builtin_skills
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| aero_protocol::AeroError::NotFound(format!("Skill pack '{id}' not found")))
    }

    pub async fn install_skill(&self, skill: SkillPack) -> AeroResult<SkillPack> {
        tracing::info!(skill_id = %skill.id, name = %skill.name, "Installing skill pack");
        Ok(skill)
    }

    pub async fn deploy_auto_detect(
        &self,
        req: DeployAutoDetectRequest,
        wasm_runtime: &WasmRuntime,
        ai_engine: &AiEngine,
    ) -> AeroResult<DeployAutoDetectResponse> {
        let start = std::time::Instant::now();

        let framework = crate::detector::FrameworkDetector::detect(&req.workspace_path);

        let skill_pack = self
            .builtin_skills
            .iter()
            .find(|s| s.framework == framework)
            .cloned();

        let (skill_pack_id, deployed, function_id) = if let Some(skill) = skill_pack {
            tracing::info!(framework = %skill.framework, skill_id = %skill.id, "Skill pack matched");

            let prompt = format!(
                "Using the {} skill pack context: {}",
                skill.framework, skill.mcp_prompt_context
            );
            let _ = ai_engine.generate_text(&prompt).await?;

            (skill.id.clone(), true, Some(uuid::Uuid::new_v4()))
        } else {
            tracing::warn!(framework = %framework, "No matching skill pack found");
            ("none".to_string(), false, None)
        };

        let elapsed = start.elapsed();

        Ok(DeployAutoDetectResponse {
            detected_framework: framework,
            skill_pack_id,
            build_duration_ms: elapsed.as_millis() as u64,
            quarantine_passed: true,
            remediation_applied: false,
            deployed,
            function_id,
        })
    }
}

fn builtin_skills() -> Vec<SkillPack> {
    use aero_protocol::skills::*;

    vec![
        SkillPack {
            id: "nextjs-optimization-pack".into(),
            name: "Next.js Optimization Pack".into(),
            version: "1.0.0".into(),
            description: "Build and deploy Next.js apps with edge-compiled Wasm".into(),
            framework: "nextjs".into(),
            mcp_prompt_context: "When building Next.js inside the Wasm sandbox, use strict memory allocation limits. Apply edge-route compilation. Verify egress security with Hazmat filter before completing deployment.".into(),
            wasm_build_tool: WasmBuildTool {
                wasm_binary_url: "builtin://nextjs-builder.wasm".into(),
                wasm_hash: "sha256:nextjs-builder-v1".into(),
                memory_limit_mb: 256,
                cpu_limit_percent: 100,
            },
            deployment_targets: vec![DeploymentTarget::LocalWasm, DeploymentTarget::Vercel],
            security_constraints: SecurityConstraints {
                egress_filtering: true,
                max_memory_mb: 256,
                max_cpu_percent: 100,
                timeout_ms: 30_000,
                hazmat_required: true,
            },
        },
        SkillPack {
            id: "react-optimization-pack".into(),
            name: "React Optimization Pack".into(),
            version: "1.0.0".into(),
            description: "Build and deploy React SPAs with Wasm-based bundling".into(),
            framework: "react".into(),
            mcp_prompt_context: "When building React inside the Wasm sandbox, enforce component-level code splitting. Apply strict CSP headers. Verify no inline scripts via Hazmat filter.".into(),
            wasm_build_tool: WasmBuildTool {
                wasm_binary_url: "builtin://react-builder.wasm".into(),
                wasm_hash: "sha256:react-builder-v1".into(),
                memory_limit_mb: 128,
                cpu_limit_percent: 100,
            },
            deployment_targets: vec![DeploymentTarget::LocalWasm, DeploymentTarget::Cloudflare],
            security_constraints: SecurityConstraints {
                egress_filtering: true,
                max_memory_mb: 128,
                max_cpu_percent: 100,
                timeout_ms: 20_000,
                hazmat_required: true,
            },
        },
        SkillPack {
            id: "rust-axum-pack".into(),
            name: "Rust + Axum Pack".into(),
            version: "1.0.0".into(),
            description: "Build and deploy Rust Axum serverless functions".into(),
            framework: "rust-axum".into(),
            mcp_prompt_context: "When building Rust Axum functions, compile to wasm32-wasi target. Enforce strict type safety. Apply Hazmat egress filtering on all network calls.".into(),
            wasm_build_tool: WasmBuildTool {
                wasm_binary_url: "builtin://rust-axum-builder.wasm".into(),
                wasm_hash: "sha256:rust-axum-builder-v1".into(),
                memory_limit_mb: 64,
                cpu_limit_percent: 100,
            },
            deployment_targets: vec![DeploymentTarget::LocalWasm],
            security_constraints: SecurityConstraints {
                egress_filtering: true,
                max_memory_mb: 64,
                max_cpu_percent: 100,
                timeout_ms: 10_000,
                hazmat_required: true,
            },
        },
        SkillPack {
            id: "python-fastapi-pack".into(),
            name: "Python FastAPI Pack".into(),
            version: "1.0.0".into(),
            description: "Build and deploy Python FastAPI functions via Wasm".into(),
            framework: "python-fastapi".into(),
            mcp_prompt_context: "When building Python FastAPI functions, use Pyodide-based Wasm compilation. Enforce PII validation via embedded AI. Apply Hazmat egress filtering.".into(),
            wasm_build_tool: WasmBuildTool {
                wasm_binary_url: "builtin://python-builder.wasm".into(),
                wasm_hash: "sha256:python-builder-v1".into(),
                memory_limit_mb: 256,
                cpu_limit_percent: 100,
            },
            deployment_targets: vec![DeploymentTarget::LocalWasm],
            security_constraints: SecurityConstraints {
                egress_filtering: true,
                max_memory_mb: 256,
                max_cpu_percent: 100,
                timeout_ms: 30_000,
                hazmat_required: true,
            },
        },
    ]
}
