use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitializeRequest {
    pub protocol_version: String,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInitializeResponse {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
    pub sampling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum McpRequest {
    ListTools,
    CallTool {
        name: String,
        arguments: serde_json::Value,
    },
    ListResources,
    ReadResource {
        uri: String,
    },
    CreateCollection {
        name: String,
        schema: serde_json::Value,
    },
    ReadDbSchema,
    GenerateFunction {
        description: String,
        language: String,
    },
    DeployFunction {
        function_id: Uuid,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}
