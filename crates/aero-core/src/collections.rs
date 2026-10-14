//! Workspace tables ("collections"): typed columns, rows stored as JSON in PostgreSQL.
//! Used by the REST API (`/v1/collections`) and by MCP tools.

use aero_db::db_err;
use aero_db::is_unique_violation;
use aero_protocol::audit::AuditAction;
use aero_protocol::keys::Scope;
use aero_protocol::tenancy::Role;
use aero_protocol::{AeroError, AeroResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::app::AeroApp;
use crate::audit::{self, Entry, Who};
use crate::auth::{ClientInfo, Ctx};

const MAX_COLUMNS: usize = 100;
const MAX_ROWS_PER_INSERT: usize = 500;
const MAX_ROW_BYTES: usize = 64 * 1024;
const MAX_COLLECTIONS: i64 = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    String,
    Integer,
    Number,
    Boolean,
    Datetime,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ColumnType,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub columns: Vec<Column>,
    pub row_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCollection {
    pub name: String,
    pub columns: Vec<Column>,
}

fn bad(m: impl Into<String>) -> AeroError {
    AeroError::BadRequest(m.into())
}

fn valid_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_ascii_lowercase())
        && s.len() <= 63
        && c.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn from_row(r: &PgRow) -> AeroResult<Collection> {
    let cols: sqlx::types::Json<Vec<Column>> = r.get("columns");
    Ok(Collection {
        id: r.get("id"),
        name: r.get("name"),
        columns: cols.0,
        row_count: r.get("row_count"),
        created_at: r.get("created_at"),
    })
}

const SELECT: &str = "SELECT c.id, c.name, c.columns, c.created_at,
    (SELECT COUNT(*) FROM collection_rows r WHERE r.collection_id = c.id) AS row_count FROM collections c";

/// Keys with a database allow-list only see the collections on it.
fn visible(ctx: &Ctx, id: Uuid) -> bool {
    ctx.key()
        .map(|k| k.allowed_database_ids.is_empty() || k.allowed_database_ids.contains(&id))
        .unwrap_or(true)
}

/// Check one row against the columns. Unknown fields are rejected.
pub fn validate_row(columns: &[Column], row: &Value) -> AeroResult<Value> {
    let obj = row
        .as_object()
        .ok_or_else(|| bad("Each row must be a JSON object"))?;
    if serde_json::to_vec(row).map(|v| v.len()).unwrap_or(0) > MAX_ROW_BYTES {
        return Err(bad("A row is larger than 64 KB"));
    }
    for key in obj.keys() {
        if !columns.iter().any(|c| &c.name == key) {
            return Err(bad(format!("Unknown column `{key}`")));
        }
    }
    for c in columns {
        match obj.get(&c.name) {
            None | Some(Value::Null) => {
                if c.required {
                    return Err(bad(format!("Column `{}` is required", c.name)));
                }
            }
            Some(v) => {
                let ok = match c.kind {
                    ColumnType::String => v.is_string(),
                    ColumnType::Integer => v.is_i64() || v.is_u64(),
                    ColumnType::Number => v.is_number(),
                    ColumnType::Boolean => v.is_boolean(),
                    ColumnType::Datetime => v
                        .as_str()
                        .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok()),
                    ColumnType::Json => true,
                };
                if !ok {
                    return Err(bad(format!(
                        "Column `{}` must be {}",
                        c.name,
                        match c.kind {
                            ColumnType::String => "a string",
                            ColumnType::Integer => "an integer",
                            ColumnType::Number => "a number",
                            ColumnType::Boolean => "true or false",
                            ColumnType::Datetime => "an RFC 3339 date-time string",
                            ColumnType::Json => "JSON",
                        }
                    )));
                }
            }
        }
    }
    Ok(row.clone())
}

pub async fn create(
    app: &AeroApp,
    ctx: &Ctx,
    client: &ClientInfo,
    req: CreateCollection,
) -> AeroResult<Collection> {
    ctx.require(Role::Manager, Scope::DatabasesWrite)?;
    let name = req.name.trim().to_string();
    if !valid_ident(&name) {
        return Err(bad(
            "Table name: start with a lowercase letter; letters, digits and _ only (max 63)",
        ));
    }
    if req.columns.is_empty() || req.columns.len() > MAX_COLUMNS {
        return Err(bad("A table needs 1–100 columns"));
    }
    let mut seen = std::collections::HashSet::new();
    for c in &req.columns {
        if !valid_ident(&c.name) || c.name == "id" {
            return Err(bad(format!(
                "Invalid column name `{}` (lowercase letters, digits, _; `id` is reserved)",
                c.name
            )));
        }
        if !seen.insert(c.name.clone()) {
            return Err(bad(format!("Column `{}` appears twice", c.name)));
        }
    }
    let count: i64 = sqlx::query("SELECT COUNT(*) AS n FROM collections WHERE workspace_id = $1")
        .bind(ctx.ws)
        .fetch_one(app.db.pool())
        .await
        .map_err(db_err)?
        .get("n");
    if count >= MAX_COLLECTIONS {
        return Err(AeroError::Conflict(
            "This workspace already has 200 tables".into(),
        ));
    }
    let id = Uuid::new_v4();
    let r = sqlx::query(
        "INSERT INTO collections (id, workspace_id, name, columns, created_by) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(id)
    .bind(ctx.ws)
    .bind(&name)
    .bind(sqlx::types::Json(&req.columns))
    .bind(ctx.actor_id())
    .execute(app.db.pool())
    .await;
    if let Err(e) = r {
        if is_unique_violation(&e) {
            return Err(AeroError::Conflict(format!(
                "Table `{name}` already exists"
            )));
        }
        return Err(db_err(e));
    }
    audit::record(
        app,
        client,
        Entry::new(
            Some(ctx.ws),
            Who::from(ctx),
            AuditAction::CollectionCreated,
            "collection",
        )
        .id(id)
        .details(json!({ "name": name, "columns": req.columns.len() })),
    )
    .await;
    get(app, ctx, &name).await
}

pub async fn list(app: &AeroApp, ctx: &Ctx) -> AeroResult<Vec<Collection>> {
    ctx.require(Role::User, Scope::DatabasesRead)?;
    let rows = sqlx::query(&format!(
        "{SELECT} WHERE c.workspace_id = $1 ORDER BY c.name"
    ))
    .bind(ctx.ws)
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?;
    let mut out = rows.iter().map(from_row).collect::<AeroResult<Vec<_>>>()?;
    out.retain(|c| visible(ctx, c.id));
    Ok(out)
}

/// A table of the caller's workspace by name (permission checks are the caller's job).
async fn find(app: &AeroApp, ctx: &Ctx, name: &str) -> AeroResult<Collection> {
    let r = sqlx::query(&format!(
        "{SELECT} WHERE c.workspace_id = $1 AND c.name = $2"
    ))
    .bind(ctx.ws)
    .bind(name)
    .fetch_optional(app.db.pool())
    .await
    .map_err(db_err)?
    .ok_or_else(|| AeroError::NotFound(format!("Table `{name}` not found")))?;
    let c = from_row(&r)?;
    if !visible(ctx, c.id) {
        return Err(AeroError::NotFound(format!("Table `{name}` not found")));
    }
    Ok(c)
}

pub async fn get(app: &AeroApp, ctx: &Ctx, name: &str) -> AeroResult<Collection> {
    ctx.require(Role::User, Scope::DatabasesRead)?;
    find(app, ctx, name).await
}

pub async fn delete(app: &AeroApp, ctx: &Ctx, client: &ClientInfo, name: &str) -> AeroResult<()> {
    ctx.require(Role::Manager, Scope::DatabasesWrite)?;
    let c = find(app, ctx, name).await?;
    sqlx::query("DELETE FROM collections WHERE id = $1")
        .bind(c.id)
        .execute(app.db.pool())
        .await
        .map_err(db_err)?;
    audit::record(
        app,
        client,
        Entry::new(
            Some(ctx.ws),
            Who::from(ctx),
            AuditAction::CollectionDeleted,
            "collection",
        )
        .id(c.id)
        .details(json!({ "name": c.name, "rows": c.row_count })),
    )
    .await;
    Ok(())
}

fn row_json(r: &PgRow) -> Value {
    json!({
        "id": r.get::<Uuid, _>("id"),
        "data": r.get::<Value, _>("data"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "updated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
    })
}

/// Insert rows (all or nothing).
pub async fn insert(
    app: &AeroApp,
    ctx: &Ctx,
    name: &str,
    rows: &[Value],
) -> AeroResult<Vec<Value>> {
    ctx.require(Role::User, Scope::DatabasesWrite)?;
    if rows.is_empty() || rows.len() > MAX_ROWS_PER_INSERT {
        return Err(bad("Send 1–500 rows"));
    }
    let c = find(app, ctx, name).await?;
    let checked = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            validate_row(&c.columns, r).map_err(|e| match e {
                AeroError::BadRequest(m) => bad(format!("Row {}: {m}", i + 1)),
                other => other,
            })
        })
        .collect::<AeroResult<Vec<_>>>()?;
    let mut tx = app.db.pool().begin().await.map_err(db_err)?;
    let mut out = Vec::with_capacity(checked.len());
    for data in checked {
        let r = sqlx::query(
            "INSERT INTO collection_rows (id, collection_id, data) VALUES ($1,$2,$3)
             RETURNING id, data, created_at, updated_at",
        )
        .bind(Uuid::new_v4())
        .bind(c.id)
        .bind(&data)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        out.push(row_json(&r));
    }
    tx.commit().await.map_err(db_err)?;
    Ok(out)
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RowQuery {
    /// Equality filter on columns, e.g. `{"status": "open"}`.
    #[serde(default, rename = "where")]
    pub filter: Option<Map<String, Value>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Rows, newest first, with the total that matches the filter.
pub async fn query(app: &AeroApp, ctx: &Ctx, name: &str, q: &RowQuery) -> AeroResult<Value> {
    ctx.require(Role::User, Scope::DatabasesRead)?;
    let c = find(app, ctx, name).await?;
    let filter = q.filter.clone().unwrap_or_default();
    for key in filter.keys() {
        if !c.columns.iter().any(|col| &col.name == key) {
            return Err(bad(format!("Unknown column `{key}` in where")));
        }
    }
    let filter = Value::Object(filter);
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).clamp(0, 1_000_000);
    let total: i64 = sqlx::query(
        "SELECT COUNT(*) AS n FROM collection_rows WHERE collection_id = $1 AND data @> $2",
    )
    .bind(c.id)
    .bind(&filter)
    .fetch_one(app.db.pool())
    .await
    .map_err(db_err)?
    .get("n");
    let rows = sqlx::query(
        "SELECT id, data, created_at, updated_at FROM collection_rows
         WHERE collection_id = $1 AND data @> $2 ORDER BY created_at DESC, id LIMIT $3 OFFSET $4",
    )
    .bind(c.id)
    .bind(&filter)
    .bind(limit)
    .bind(offset)
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?;
    Ok(json!({ "rows": rows.iter().map(row_json).collect::<Vec<_>>(), "total": total }))
}

pub async fn delete_row(app: &AeroApp, ctx: &Ctx, name: &str, id: Uuid) -> AeroResult<()> {
    ctx.require(Role::User, Scope::DatabasesWrite)?;
    let c = find(app, ctx, name).await?;
    let r = sqlx::query("DELETE FROM collection_rows WHERE id = $1 AND collection_id = $2")
        .bind(id)
        .bind(c.id)
        .execute(app.db.pool())
        .await
        .map_err(db_err)?;
    if r.rows_affected() == 0 {
        return Err(AeroError::NotFound("Row not found".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_validation() {
        let cols = vec![
            Column {
                name: "title".into(),
                kind: ColumnType::String,
                required: true,
            },
            Column {
                name: "n".into(),
                kind: ColumnType::Integer,
                required: false,
            },
            Column {
                name: "at".into(),
                kind: ColumnType::Datetime,
                required: false,
            },
        ];
        assert!(validate_row(&cols, &json!({"title": "a", "n": 2})).is_ok());
        assert!(validate_row(&cols, &json!({"n": 2})).is_err(), "required");
        assert!(
            validate_row(&cols, &json!({"title": "a", "n": 2.5})).is_err(),
            "integer"
        );
        assert!(
            validate_row(&cols, &json!({"title": "a", "x": 1})).is_err(),
            "unknown"
        );
        assert!(validate_row(&cols, &json!({"title": "a", "at": "2026-10-14T12:00:00Z"})).is_ok());
        assert!(validate_row(&cols, &json!({"title": "a", "at": "yesterday"})).is_err());
        assert!(validate_row(&cols, &json!([1])).is_err());
        assert!(valid_ident("tasks_2") && !valid_ident("2tasks") && !valid_ident("Tasks"));
    }
}
