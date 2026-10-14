//! MCP (Model Context Protocol) server — Streamable HTTP transport, JSON responses.
//!
//! `POST /v1/mcp` with one JSON-RPC 2.0 message. Authenticate like any API call (an `aero_…` key
//! with the `mcp.connect` scope, or a signed-in member). Each tool checks its own scope/role, so an
//! agent can only do what its key allows. Every POST costs one key credit.
//!
//! Client config example (Claude Code, Cursor, …):
//! `{"type":"http","url":"https://<host>/v1/mcp","headers":{"Authorization":"Bearer aero_…"}}`

use aero_protocol::functions::DeployRequest;
use aero_protocol::keys::Scope;
use aero_protocol::tenancy::Role;
use aero_protocol::AeroError;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::auth::{ClientInfo, Ctx};
use crate::collections::{self, CreateCollection, RowQuery};
use crate::router::AppState;
use crate::{functions, inspector};

pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// `GET /v1/mcp` — this server does not offer a server-to-client SSE stream.
pub async fn get_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [("allow", "POST")],
        "Use POST with a JSON-RPC message",
    )
        .into_response()
}

/// `POST /v1/mcp`
pub async fn post(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    _headers: HeaderMap,
    Json(msg): Json<Value>,
) -> Response {
    if let Err(e) = ctx.require(Role::User, Scope::McpConnect) {
        return crate::error::AeroApiError::from(e).into_response();
    }
    if msg.is_array() {
        return Json(rpc_error(Value::Null, -32600, "Batches are not supported")).into_response();
    }
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let Some(id) = msg.get("id").cloned() else {
        // Notifications (e.g. notifications/initialized) and responses: accepted, no body.
        return StatusCode::ACCEPTED.into_response();
    };
    if msg.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Json(rpc_error(id, -32600, "Expected jsonrpc 2.0")).into_response();
    }
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let body = match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(SUPPORTED_VERSIONS[0]);
            let version = if SUPPORTED_VERSIONS.contains(&asked) {
                asked
            } else {
                SUPPORTED_VERSIONS[0]
            };
            rpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "aerobaas", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "AeroBaaS workspace tools: tables (create, insert, query), Wasm functions (deploy, run) and request logs. All actions are limited to the workspace and scopes of your API key.",
                }),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            if !tools().iter().any(|t| t["name"] == name) {
                rpc_error(id, -32602, &format!("Unknown tool: {name}"))
            } else {
                let result = call_tool(&state, &ctx, &client, name, args).await;
                rpc_result(
                    id,
                    match result {
                        Ok(v) => json!({
                            "content": [{ "type": "text", "text": v.to_string() }],
                            "structuredContent": v,
                            "isError": false,
                        }),
                        Err(e) => json!({
                            "content": [{ "type": "text", "text": message(&e) }],
                            "isError": true,
                        }),
                    },
                )
            }
        }
        "resources/list" => rpc_result(id, json!({ "resources": [] })),
        "prompts/list" => rpc_result(id, json!({ "prompts": [] })),
        _ => rpc_error(id, -32601, &format!("Method not found: {method}")),
    };
    Json(body).into_response()
}

fn message(e: &AeroError) -> String {
    match e {
        AeroError::Internal(_) | AeroError::Database(_) => "Server error; try again".into(),
        AeroError::BadRequest(m)
        | AeroError::Forbidden(m)
        | AeroError::NotFound(m)
        | AeroError::Conflict(m)
        | AeroError::Unauthorized(m)
        | AeroError::TooManyRequests(m) => m.clone(),
        other => other.to_string(),
    }
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, AeroError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AeroError::BadRequest(format!("Missing argument `{key}`")))
}

async fn call_tool(
    state: &AppState,
    ctx: &Ctx,
    client: &ClientInfo,
    name: &str,
    args: Value,
) -> Result<Value, AeroError> {
    let app = &state.app;
    match name {
        "list_tables" => Ok(json!({ "tables": collections::list(app, ctx).await? })),
        "create_table" => {
            let req: CreateCollection = serde_json::from_value(args)
                .map_err(|e| AeroError::BadRequest(format!("Invalid arguments: {e}")))?;
            Ok(json!(collections::create(app, ctx, client, req).await?))
        }
        "insert_rows" => {
            let table = arg_str(&args, "table")?;
            let rows = args
                .get("rows")
                .and_then(Value::as_array)
                .ok_or_else(|| AeroError::BadRequest("Missing argument `rows` (array)".into()))?;
            let inserted = collections::insert(app, ctx, table, rows).await?;
            Ok(json!({ "inserted": inserted.len(), "rows": inserted }))
        }
        "query_rows" => {
            let table = arg_str(&args, "table")?;
            let q: RowQuery = serde_json::from_value(args.clone())
                .map_err(|e| AeroError::BadRequest(format!("Invalid arguments: {e}")))?;
            collections::query(app, ctx, table, &q).await
        }
        "list_functions" => Ok(json!({ "functions": functions::list(app, ctx).await? })),
        "deploy_function" => {
            let req = DeployRequest {
                name: arg_str(&args, "name")?.to_string(),
                wasm_bytes: vec![],
                wasm_base64: Some(arg_str(&args, "wasm_base64")?.to_string()),
                entrypoint: args
                    .get("entrypoint")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                env_vars: None,
                memory_limit_mb: None,
                cpu_limit_percent: None,
                timeout_ms: args
                    .get("timeout_ms")
                    .and_then(Value::as_u64)
                    .map(|v| v.min(u32::MAX as u64) as u32),
            };
            Ok(json!(functions::deploy(app, ctx, client, req).await?))
        }
        "run_function" => {
            let id = functions::resolve(app, ctx, arg_str(&args, "function")?).await?;
            let payload = args.get("payload").cloned().unwrap_or(Value::Null);
            let (resp, error) = functions::execute(app, ctx, client, id, payload).await?;
            let mut v = json!(resp);
            if let Some(e) = error {
                v["error"] = json!(e);
            }
            Ok(v)
        }
        "query_logs" => {
            ctx.require(Role::Manager, Scope::Admin)?;
            let q = inspector::ListQuery {
                limit: args.get("limit").and_then(Value::as_i64).or(Some(50)),
                before_id: None,
                method: None,
                status: args
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                path: args.get("path").and_then(Value::as_str).map(str::to_string),
                user_id: None,
            };
            inspector::list(app, ctx.ws, &q).await
        }
        _ => Err(AeroError::NotFound(format!("Unknown tool {name}"))),
    }
}

/// Tool definitions (JSON Schema inputs).
pub fn tools() -> Vec<Value> {
    let column = json!({
        "type": "object",
        "required": ["name", "type"],
        "properties": {
            "name": { "type": "string", "description": "lowercase letters, digits, _" },
            "type": { "type": "string", "enum": ["string", "integer", "number", "boolean", "datetime", "json"] },
            "required": { "type": "boolean" }
        }
    });
    vec![
        json!({
            "name": "list_tables",
            "description": "List the tables of this workspace with their columns and row counts. Scope: databases.read.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "create_table",
            "description": "Create a table with typed columns. Scope: databases.write (members: Manager or higher).",
            "inputSchema": {
                "type": "object",
                "required": ["name", "columns"],
                "properties": {
                    "name": { "type": "string", "description": "lowercase letters, digits, _ (max 63)" },
                    "columns": { "type": "array", "items": column, "minItems": 1 }
                }
            }
        }),
        json!({
            "name": "insert_rows",
            "description": "Insert 1–500 rows (JSON objects matching the columns). All or nothing. Scope: databases.write.",
            "inputSchema": {
                "type": "object",
                "required": ["table", "rows"],
                "properties": {
                    "table": { "type": "string" },
                    "rows": { "type": "array", "items": { "type": "object" }, "minItems": 1, "maxItems": 500 }
                }
            }
        }),
        json!({
            "name": "query_rows",
            "description": "Read rows, newest first. `where` is an equality filter on columns. Scope: databases.read.",
            "inputSchema": {
                "type": "object",
                "required": ["table"],
                "properties": {
                    "table": { "type": "string" },
                    "where": { "type": "object" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500 },
                    "offset": { "type": "integer", "minimum": 0 }
                }
            },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "list_functions",
            "description": "List the Wasm functions of this workspace. Scope: functions.list or functions.read.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "deploy_function",
            "description": "Deploy a WASI preview 1 module (e.g. Rust built for wasm32-wasip1). The function reads its JSON payload from stdin and writes the result to stdout. Scope: functions.write.",
            "inputSchema": {
                "type": "object",
                "required": ["name", "wasm_base64"],
                "properties": {
                    "name": { "type": "string" },
                    "wasm_base64": { "type": "string", "description": "the .wasm file, base64-encoded (max 10 MB)" },
                    "entrypoint": { "type": "string", "description": "exported function to run, default _start" },
                    "timeout_ms": { "type": "integer", "minimum": 1 }
                }
            }
        }),
        json!({
            "name": "run_function",
            "description": "Run a function by id or name with a JSON payload; returns its output, logs, status and timing. Scope: executions.write.",
            "inputSchema": {
                "type": "object",
                "required": ["function"],
                "properties": {
                    "function": { "type": "string", "description": "function id or name" },
                    "payload": {}
                }
            }
        }),
        json!({
            "name": "query_logs",
            "description": "Recent HTTP requests in this workspace (method, path, status, latency). Scope: admin (members: Manager or higher).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500 },
                    "status": { "type": "string", "enum": ["2xx", "3xx", "4xx", "5xx", "errors"] },
                    "path": { "type": "string" }
                }
            },
            "annotations": { "readOnlyHint": true }
        }),
    ]
}
