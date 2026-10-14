use std::sync::Arc;

use aero_ai_engine::AiEngine;
use aero_db::DbClient;
use aero_protocol::mcp::{
    McpInitializeRequest, McpInitializeResponse, McpResource, McpTool, ServerCapabilities,
    ServerInfo,
};
use aero_wasm_runtime::WasmRuntime;
use serde_json::json;

pub struct McpServer {
    db: Arc<DbClient>,
    ai_engine: Arc<AiEngine>,
    #[allow(dead_code)] // used by MCP tools in Phase 2
    wasm_runtime: Arc<WasmRuntime>,
}

impl McpServer {
    pub fn new(
        db: Arc<DbClient>,
        ai_engine: Arc<AiEngine>,
        wasm_runtime: Arc<WasmRuntime>,
    ) -> Self {
        Self {
            db,
            ai_engine,
            wasm_runtime,
        }
    }

    pub fn initialize(&self, _req: McpInitializeRequest) -> McpInitializeResponse {
        McpInitializeResponse {
            protocol_version: "2024-11-05".into(),
            server_info: ServerInfo {
                name: "AeroBaaS MCP Server".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: ServerCapabilities {
                tools: true,
                resources: true,
                prompts: true,
                sampling: true,
            },
        }
    }

    pub fn list_tools(&self) -> Vec<McpTool> {
        crate::tools::all_tools()
    }

    pub fn list_resources(&self) -> Vec<McpResource> {
        vec![
            McpResource {
                uri: "aero://db/schema".into(),
                name: "Database Schema".into(),
                description: "Current database schema and collections".into(),
                mime_type: "application/json".into(),
            },
            McpResource {
                uri: "aero://functions".into(),
                name: "Deployed Functions".into(),
                description: "List of all deployed Wasm functions".into(),
                mime_type: "application/json".into(),
            },
            McpResource {
                uri: "aero://skills".into(),
                name: "Installed Skills".into(),
                description: "Available skill packs for framework deployment".into(),
                mime_type: "application/json".into(),
            },
        ]
    }

    pub async fn read_resource(&self, uri: &str) -> anyhow::Result<serde_json::Value> {
        match uri {
            "aero://db/schema" => {
                let schema = self.db.get_schema().await?;
                Ok(schema)
            }
            // Needs the caller's workspace; arrives with the MCP protocol in Phase 2.
            "aero://functions" => anyhow::bail!("aero://functions is not available yet (Phase 2)"),
            _ => anyhow::bail!("Unknown resource URI: {uri}"),
        }
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        match name {
            "read_db_schema" => {
                let schema = self.db.get_schema().await?;
                Ok(schema)
            }
            "create_collection" => {
                let coll_name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' field"))?;
                let schema = arguments.get("schema").cloned().unwrap_or(json!({}));
                self.db.create_collection(coll_name, schema).await?;
                Ok(json!({ "created": true, "collection": coll_name }))
            }
            "generate_function" => {
                let description = arguments
                    .get("description")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'description' field"))?;
                let language = arguments
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("rust");
                let generated = self
                    .ai_engine
                    .generate_text(&format!(
                        "Generate a {language} serverless function: {description}"
                    ))
                    .await?;
                Ok(json!({ "code": generated, "language": language }))
            }
            "ai_generate" => {
                let prompt = arguments
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'prompt' field"))?;
                let result = self.ai_engine.generate_text(prompt).await?;
                Ok(json!({ "text": result }))
            }
            _ => anyhow::bail!("Unknown tool: {name}"),
        }
    }

    pub async fn handle_connection(&self, socket: axum::extract::ws::WebSocket) {
        tracing::info!("MCP WebSocket connection established");
        let _ = socket;
        // WebSocket handling will be implemented with the MCP JSON-RPC protocol
    }
}
