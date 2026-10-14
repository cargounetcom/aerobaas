//! Functions: deploy, list, run, execution history. Used by the REST API and by MCP tools.

use aero_db::db_err;
use aero_keys::KeyManager;
use aero_protocol::audit::{AuditAction, EventSeverity};
use aero_protocol::functions::{DeployRequest, DeployResponse, ExecuteResponse, Function};
use aero_protocol::keys::Scope;
use aero_protocol::tenancy::Role;
use aero_protocol::{AeroError, AeroResult};
use base64::Engine as _;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::app::AeroApp;
use crate::audit::{self, Entry, Who};
use crate::auth::{ClientInfo, Ctx};

/// Keys with a function allow-list only see the functions on it (404 for the rest).
fn visible(ctx: &Ctx, id: Uuid) -> AeroResult<()> {
    match ctx.key() {
        Some(k) if !KeyManager::function_allowed(k, id) => {
            Err(AeroError::NotFound(format!("Function {id} not found")))
        }
        _ => Ok(()),
    }
}

pub async fn deploy(
    app: &AeroApp,
    ctx: &Ctx,
    client: &ClientInfo,
    req: DeployRequest,
) -> AeroResult<DeployResponse> {
    ctx.require(Role::User, Scope::FunctionsWrite)?;
    let wasm = match req.wasm_base64.as_deref() {
        Some(b64) if !b64.trim().is_empty() => base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|_| AeroError::BadRequest("wasm_base64 is not valid base64".into()))?,
        _ => req.wasm_bytes.clone(),
    };
    let (function, resp) = app
        .wasm_runtime
        .deploy(ctx.ws, &req, &wasm, &app.remediation)
        .await?;
    app.db
        .store_function(&function, &wasm, ctx.user_id())
        .await?;
    audit::record(
        app,
        client,
        Entry::new(
            Some(ctx.ws),
            Who::from(ctx),
            AuditAction::FunctionDeployed,
            "function",
        )
        .id(function.id)
        .details(json!({
            "name": function.name,
            "wasm_hash": function.wasm_hash,
            "size_bytes": wasm.len(),
        })),
    )
    .await;
    Ok(resp)
}

pub async fn list(app: &AeroApp, ctx: &Ctx) -> AeroResult<Vec<Function>> {
    if ctx.require(Role::User, Scope::FunctionsList).is_err() {
        ctx.require(Role::User, Scope::FunctionsRead)?;
    }
    let mut funcs = app.db.list_functions(ctx.ws).await?;
    if let Some(k) = ctx.key() {
        funcs.retain(|f| KeyManager::function_allowed(k, f.id));
    }
    Ok(funcs)
}

pub async fn get(app: &AeroApp, ctx: &Ctx, id: Uuid) -> AeroResult<Function> {
    ctx.require(Role::User, Scope::FunctionsRead)?;
    visible(ctx, id)?;
    app.db.get_function(ctx.ws, id).await
}

/// Find a function by id or by exact name (for MCP).
pub async fn resolve(app: &AeroApp, ctx: &Ctx, id_or_name: &str) -> AeroResult<Uuid> {
    if let Ok(id) = Uuid::parse_str(id_or_name) {
        return Ok(id);
    }
    let funcs = app.db.list_functions(ctx.ws).await?;
    funcs
        .into_iter()
        .find(|f| f.name == id_or_name)
        .map(|f| f.id)
        .ok_or_else(|| AeroError::NotFound(format!("Function `{id_or_name}` not found")))
}

pub async fn delete(app: &AeroApp, ctx: &Ctx, client: &ClientInfo, id: Uuid) -> AeroResult<()> {
    ctx.require(Role::Manager, Scope::FunctionsWrite)?;
    visible(ctx, id)?;
    if !app.db.delete_function(ctx.ws, id).await? {
        return Err(AeroError::NotFound(format!("Function {id} not found")));
    }
    audit::record(
        app,
        client,
        Entry::new(
            Some(ctx.ws),
            Who::from(ctx),
            AuditAction::FunctionDeleted,
            "function",
        )
        .id(id),
    )
    .await;
    Ok(())
}

/// Run a function and record the execution. Sandbox failures are returned as a response with a
/// non-`ok` status (the HTTP layer maps them to 422).
pub async fn execute(
    app: &AeroApp,
    ctx: &Ctx,
    client: &ClientInfo,
    id: Uuid,
    payload: Value,
) -> AeroResult<(ExecuteResponse, Option<String>)> {
    ctx.require(Role::User, Scope::ExecutionsWrite)?;
    visible(ctx, id)?;
    let function = app.db.get_function(ctx.ws, id).await?;
    let wasm = app.db.get_function_wasm(ctx.ws, id).await?;
    let outcome = app.wasm_runtime.execute(&function, wasm, payload).await?;
    let execution_id = Uuid::new_v4();
    let r = sqlx::query(
        "INSERT INTO executions (id, workspace_id, function_id, user_id, key_id, status, duration_ms,
            fuel_used, memory_mb, cold_start, error)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(execution_id)
    .bind(ctx.ws)
    .bind(id)
    .bind(ctx.user_id())
    .bind(ctx.key().map(|k| k.id))
    .bind(outcome.status.as_str())
    .bind(outcome.duration_ms.min(i32::MAX as u64) as i32)
    .bind(outcome.fuel_used.min(i64::MAX as u64) as i64)
    .bind(outcome.memory_mb)
    .bind(outcome.cold_start)
    .bind(&outcome.error)
    .execute(app.db.pool())
    .await;
    if let Err(e) = r {
        tracing::warn!(error = %e, "failed to record execution");
    }
    let ok = outcome.status == aero_wasm_runtime::RunStatus::Ok;
    audit::record(
        app,
        client,
        Entry::new(
            Some(ctx.ws),
            Who::from(ctx),
            AuditAction::FunctionExecuted,
            "function",
        )
        .id(id)
        .details(json!({
            "execution_id": execution_id,
            "status": outcome.status.as_str(),
            "duration_ms": outcome.duration_ms,
        }))
        .severity(if ok {
            EventSeverity::Info
        } else {
            EventSeverity::Warning
        }),
    )
    .await;
    let error = outcome.error.clone();
    Ok((outcome.into_response(execution_id), error))
}

/// Recent executions of one function.
pub async fn executions(app: &AeroApp, ctx: &Ctx, id: Uuid, limit: i64) -> AeroResult<Vec<Value>> {
    ctx.require(Role::User, Scope::ExecutionsRead)?;
    visible(ctx, id)?;
    app.db.get_function(ctx.ws, id).await?;
    let rows = sqlx::query(
        "SELECT id, status, duration_ms, fuel_used, memory_mb, cold_start, error, user_id, key_id, ts
         FROM executions WHERE workspace_id = $1 AND function_id = $2 ORDER BY ts DESC LIMIT $3",
    )
    .bind(ctx.ws)
    .bind(id)
    .bind(limit.clamp(1, 500))
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?;
    Ok(rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "status": r.get::<String, _>("status"),
                "duration_ms": r.get::<i32, _>("duration_ms"),
                "fuel_used": r.get::<i64, _>("fuel_used"),
                "memory_mb": r.get::<f64, _>("memory_mb"),
                "cold_start": r.get::<bool, _>("cold_start"),
                "error": r.get::<Option<String>, _>("error"),
                "user_id": r.get::<Option<Uuid>, _>("user_id"),
                "key_id": r.get::<Option<Uuid>, _>("key_id"),
                "ts": r.get::<chrono::DateTime<chrono::Utc>, _>("ts"),
            })
        })
        .collect())
}
