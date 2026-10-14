use aero_protocol::mcp::McpTool;
use serde_json::json;

pub fn all_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "read_db_schema".into(),
            description: "Read the current database schema and all collections".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        McpTool {
            name: "create_collection".into(),
            description: "Create a new database collection with an optional schema".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Collection name" },
                    "schema": { "type": "object", "description": "Collection schema definition" }
                },
                "required": ["name"]
            }),
        },
        McpTool {
            name: "generate_function".into(),
            description: "Generate Wasm serverless function code using the embedded AI engine"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "Natural language description of the function" },
                    "language": { "type": "string", "description": "Programming language (rust, typescript, go)" }
                },
                "required": ["description"]
            }),
        },
        McpTool {
            name: "deploy_function".into(),
            description: "Deploy a Wasm function to the serverless runtime".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "wasm_bytes": { "type": "string", "description": "Base64-encoded Wasm binary" },
                    "entrypoint": { "type": "string" }
                },
                "required": ["name", "wasm_bytes", "entrypoint"]
            }),
        },
        McpTool {
            name: "ai_generate".into(),
            description: "Generate text using the embedded local SLM (no external API calls)"
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" }
                },
                "required": ["prompt"]
            }),
        },
        McpTool {
            name: "scan_dependencies".into(),
            description: "Scan project dependencies for known CVE vulnerabilities".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "manifest_type": { "type": "string", "enum": ["package_json", "cargo_toml", "go_mod"] },
                    "manifest_content": { "type": "string" }
                },
                "required": ["manifest_type", "manifest_content"]
            }),
        },
        McpTool {
            name: "list_functions".into(),
            description: "List all deployed Wasm functions".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        McpTool {
            name: "list_skills".into(),
            description: "List all installed skill packs for framework deployment".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
    ]
}
