//! PostgreSQL storage (sqlx). The schema lives in `migrations/` and is applied at startup.

use std::time::Duration;

use aero_config::DatabaseConfig;
use aero_protocol::functions::Function;
use aero_protocol::keys::ApiKey;
use aero_protocol::{AeroError, AeroResult};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub struct DbClient {
    pool: PgPool,
}

/// Map any sqlx error to a 503 without leaking details to clients (details go to the log).
pub fn db_err(e: sqlx::Error) -> AeroError {
    tracing::error!(error = %e, "database error");
    AeroError::Database("database unavailable".into())
}

/// True when the error is a unique-constraint violation.
pub fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.code().as_deref() == Some("23505"))
}

/// Serialise a snake_case enum to its string form ("owner", "deployed", …).
pub fn enc<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// Parse a snake_case enum from its stored string form.
pub fn dec<T: DeserializeOwned>(s: &str) -> AeroResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| AeroError::Internal(format!("unexpected stored value `{s}`")))
}

fn key_from_row(r: &PgRow) -> AeroResult<ApiKey> {
    let scopes: Vec<String> = r.get("scopes");
    Ok(ApiKey {
        id: r.get("id"),
        project_id: r.get("workspace_id"),
        key_prefix: r.get("key_prefix"),
        key_hash: r.get("key_hash"),
        name: r.get("name"),
        scopes: scopes.iter().filter_map(|s| dec(s).ok()).collect(),
        allowed_function_ids: r.get("allowed_function_ids"),
        allowed_database_ids: r.get("allowed_database_ids"),
        credits: r.get("credits"),
        credits_limit: r.get("credits_limit"),
        enabled: r.get("enabled"),
        created_at: r.get("created_at"),
        expires_at: r.get("expires_at"),
    })
}

fn function_from_row(r: &PgRow) -> AeroResult<Function> {
    Ok(Function {
        id: r.get("id"),
        project_id: r.get("workspace_id"),
        name: r.get("name"),
        runtime: dec(r.get::<&str, _>("runtime"))?,
        wasm_hash: r.get("wasm_hash"),
        entrypoint: r.get("entrypoint"),
        env_vars: vec![],
        memory_limit_mb: r.get::<i32, _>("memory_limit_mb") as u32,
        cpu_limit_percent: r.get::<i32, _>("cpu_limit_percent") as u32,
        timeout_ms: r.get::<i32, _>("timeout_ms") as u32,
        status: dec(r.get::<&str, _>("status"))?,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}

const KEY_COLS: &str = "id, workspace_id, key_prefix, key_hash, name, scopes, allowed_function_ids, allowed_database_ids, credits, credits_limit, enabled, created_at, expires_at";
const FN_COLS: &str = "id, workspace_id, name, runtime, wasm_hash, entrypoint, memory_limit_mb, cpu_limit_percent, timeout_ms, status, created_at, updated_at";

impl DbClient {
    /// Connect and apply pending migrations.
    pub async fn new(config: &DatabaseConfig) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections.max(1))
            .acquire_timeout(Duration::from_millis(
                config.connection_timeout_ms.max(1000),
            ))
            .connect(&config.url)
            .await
            .map_err(|e| anyhow::anyhow!("cannot connect to PostgreSQL: {e}"))?;
        MIGRATOR
            .run(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("database migration failed: {e}"))?;
        tracing::info!("PostgreSQL connected, migrations applied");
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ------------------------------------------------------------------ API keys

    pub async fn store_key(&self, key: &ApiKey, created_by: Option<Uuid>) -> AeroResult<()> {
        let scopes: Vec<String> = key.scopes.iter().map(enc).collect();
        sqlx::query(
            "INSERT INTO api_keys (id, workspace_id, created_by, name, key_prefix, key_hash, scopes,
                allowed_function_ids, allowed_database_ids, credits, credits_limit, enabled, created_at, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(key.id)
        .bind(key.project_id)
        .bind(created_by)
        .bind(&key.name)
        .bind(&key.key_prefix)
        .bind(&key.key_hash)
        .bind(&scopes)
        .bind(&key.allowed_function_ids)
        .bind(&key.allowed_database_ids)
        .bind(key.credits)
        .bind(key.credits_limit)
        .bind(key.enabled)
        .bind(key.created_at)
        .bind(key.expires_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn get_key_by_hash(&self, hash: &str) -> AeroResult<Option<ApiKey>> {
        let row = sqlx::query(&format!(
            "SELECT {KEY_COLS} FROM api_keys WHERE key_hash = $1"
        ))
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        row.as_ref().map(key_from_row).transpose()
    }

    pub async fn get_key(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<Option<ApiKey>> {
        let row = sqlx::query(&format!(
            "SELECT {KEY_COLS} FROM api_keys WHERE id = $1 AND workspace_id = $2"
        ))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        row.as_ref().map(key_from_row).transpose()
    }

    pub async fn list_keys(&self, workspace_id: Uuid) -> AeroResult<Vec<ApiKey>> {
        let rows = sqlx::query(&format!(
            "SELECT {KEY_COLS} FROM api_keys WHERE workspace_id = $1 ORDER BY created_at DESC"
        ))
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(key_from_row).collect()
    }

    /// Returns false when no key with that id exists in the workspace.
    pub async fn delete_key(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<bool> {
        let r = sqlx::query("DELETE FROM api_keys WHERE id = $1 AND workspace_id = $2")
            .bind(id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.rows_affected() > 0)
    }

    /// Atomically take `amount` credits. Returns the remaining balance, or `None` when the key
    /// had too few credits (the key is then disabled).
    pub async fn take_credits(&self, key_id: Uuid, amount: i64) -> AeroResult<Option<i64>> {
        let row = sqlx::query(
            "UPDATE api_keys SET credits = credits - $2, last_used_at = now(),
                    enabled = (credits - $2) > 0
             WHERE id = $1 AND enabled AND credits >= $2
             RETURNING credits",
        )
        .bind(key_id)
        .bind(amount)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        if let Some(r) = row {
            return Ok(Some(r.get("credits")));
        }
        sqlx::query("UPDATE api_keys SET enabled = FALSE WHERE id = $1 AND credits < $2")
            .bind(key_id)
            .bind(amount)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(None)
    }

    /// Ids from `ids` that are functions of this workspace.
    pub async fn existing_function_ids(
        &self,
        workspace_id: Uuid,
        ids: &[Uuid],
    ) -> AeroResult<Vec<Uuid>> {
        let rows = sqlx::query("SELECT id FROM functions WHERE workspace_id = $1 AND id = ANY($2)")
            .bind(workspace_id)
            .bind(ids)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(rows.iter().map(|r| r.get("id")).collect())
    }

    // ------------------------------------------------------------------ functions

    pub async fn store_function(
        &self,
        func: &Function,
        wasm: &[u8],
        created_by: Option<Uuid>,
    ) -> AeroResult<()> {
        sqlx::query(
            "INSERT INTO functions (id, workspace_id, name, runtime, wasm_hash, wasm, entrypoint,
                memory_limit_mb, cpu_limit_percent, timeout_ms, status, created_by, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(func.id)
        .bind(func.project_id)
        .bind(&func.name)
        .bind(enc(&func.runtime))
        .bind(&func.wasm_hash)
        .bind(wasm)
        .bind(&func.entrypoint)
        .bind(func.memory_limit_mb as i32)
        .bind(func.cpu_limit_percent as i32)
        .bind(func.timeout_ms as i32)
        .bind(enc(&func.status))
        .bind(created_by)
        .bind(func.created_at)
        .bind(func.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub async fn get_function(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<Function> {
        let row = sqlx::query(&format!(
            "SELECT {FN_COLS} FROM functions WHERE id = $1 AND workspace_id = $2"
        ))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .ok_or_else(|| AeroError::NotFound(format!("Function {id} not found")))?;
        function_from_row(&row)
    }

    pub async fn get_function_wasm(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<Vec<u8>> {
        let row = sqlx::query("SELECT wasm FROM functions WHERE id = $1 AND workspace_id = $2")
            .bind(id)
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AeroError::NotFound(format!("Function {id} not found")))?;
        Ok(row.get("wasm"))
    }

    pub async fn list_functions(&self, workspace_id: Uuid) -> AeroResult<Vec<Function>> {
        let rows = sqlx::query(&format!(
            "SELECT {FN_COLS} FROM functions WHERE workspace_id = $1 ORDER BY created_at DESC"
        ))
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(function_from_row).collect()
    }

    pub async fn delete_function(&self, workspace_id: Uuid, id: Uuid) -> AeroResult<bool> {
        let r = sqlx::query("DELETE FROM functions WHERE id = $1 AND workspace_id = $2")
            .bind(id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.rows_affected() > 0)
    }
}
