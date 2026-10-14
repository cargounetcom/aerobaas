//! Request inspector: every HTTP request is recorded in PostgreSQL (method, path, status,
//! latency, user/key, workspace, client). Bodies and query strings are never stored, and invite
//! tokens are removed from paths.

use std::net::SocketAddr;
use std::time::Instant;

use aero_db::db_err;
use aero_protocol::inspector::{InspectorQuery, InspectorStats, RequestTrace};
use aero_protocol::AeroResult;
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::app::AeroApp;
use crate::auth::{client_ip, MetaHandle};
use crate::router::AppState;

/// Replace secrets in paths before they are stored (`/api/invites/{token}…`, `/v1/keys/verify` has
/// the key in the body, so nothing to do there).
pub fn sanitize_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut out = Vec::with_capacity(parts.len());
    let mut i = 0;
    while i < parts.len() {
        out.push(parts[i].to_string());
        if parts[i] == "invites" && i >= 1 && parts[i - 1] == "api" && i + 1 < parts.len() {
            out.push(":token".to_string());
            i += 2;
            continue;
        }
        i += 1;
    }
    out.join("/").chars().take(500).collect()
}

/// `/v1/workspaces/{id}/…` and `/api/workspaces/{id}/…`
pub fn workspace_from_path(path: &str) -> Option<Uuid> {
    let mut it = path.split('/');
    while let Some(seg) = it.next() {
        if seg == "workspaces" {
            return it.next().and_then(|s| Uuid::parse_str(s).ok());
        }
    }
    None
}

/// Middleware around every route.
pub async fn record(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let raw_path = req.uri().path().to_string();
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    let ip = client_ip(req.headers(), peer, state.app.config.server.trust_proxy);
    let ua = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(300).collect::<String>());
    let meta = MetaHandle::default();
    req.extensions_mut().insert(meta.clone());

    let res = next.run(req).await;

    let status = res.status().as_u16() as i32;
    let latency = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
    let m = meta.snapshot();
    let workspace_id = workspace_from_path(&raw_path).or(m.workspace_id);
    let path = sanitize_path(&raw_path);
    let pool = state.app.db.pool().clone();
    tokio::spawn(async move {
        let r = sqlx::query(
            "INSERT INTO requests (method, path, status, latency_ms, user_id, key_id, workspace_id, ip, user_agent, detail)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(method)
        .bind(path)
        .bind(status)
        .bind(latency)
        .bind(m.user_id)
        .bind(m.key_id)
        .bind(workspace_id)
        .bind(ip)
        .bind(ua)
        .bind(m.detail)
        .execute(&pool)
        .await;
        if let Err(e) = r {
            tracing::warn!(error = %e, "failed to record request");
        }
    });
    res
}

/// Delete inspector rows older than the retention period (runs hourly).
pub fn spawn_retention(app: std::sync::Arc<AeroApp>) {
    let days = app.config.server.inspector_retention_days.max(1) as i32;
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            let r =
                sqlx::query("DELETE FROM requests WHERE ts < now() - make_interval(days => $1)")
                    .bind(days)
                    .execute(app.db.pool())
                    .await;
            match r {
                Ok(done) if done.rows_affected() > 0 => {
                    tracing::info!(deleted = done.rows_affected(), "inspector retention")
                }
                Err(e) => tracing::warn!(error = %e, "inspector retention failed"),
                _ => {}
            }
        }
    });
}

// ---------------------------------------------------------------- SovereignBaaS admin shapes

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub before_id: Option<i64>,
    pub method: Option<String>,
    /// "2xx" | "3xx" | "4xx" | "5xx" | "errors"
    pub status: Option<String>,
    pub path: Option<String>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRow {
    pub id: i64,
    pub ts: String,
    pub method: String,
    pub path: String,
    pub status: i64,
    pub latency_ms: i64,
    pub user_id: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub detail: Option<String>,
}

fn status_range(s: Option<&str>) -> (i32, i32) {
    match s {
        Some("2xx") => (200, 299),
        Some("3xx") => (300, 399),
        Some("4xx") => (400, 499),
        Some("5xx") => (500, 999),
        Some("errors") => (400, 999),
        _ => (0, 999),
    }
}

/// `GET /api/workspaces/{ws}/inspector/requests` → `{items, nextBeforeId}`
pub async fn list(app: &AeroApp, ws: Uuid, q: &ListQuery) -> AeroResult<Value> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let (lo, hi) = status_range(q.status.as_deref());
    let method = q
        .method
        .as_deref()
        .filter(|m| !m.is_empty())
        .map(|m| m.to_ascii_uppercase());
    let path = q
        .path
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(|p| format!("%{}%", p.replace(['%', '_'], "")));
    let rows = sqlx::query(
        "SELECT id, ts, method, path, status, latency_ms, user_id, ip, user_agent, detail FROM requests
         WHERE workspace_id = $1 AND ($2::bigint IS NULL OR id < $2) AND ($3::text IS NULL OR method = $3)
           AND status BETWEEN $4 AND $5 AND ($6::text IS NULL OR path LIKE $6) AND ($7::uuid IS NULL OR user_id = $7)
         ORDER BY id DESC LIMIT $8",
    )
    .bind(ws)
    .bind(q.before_id)
    .bind(method)
    .bind(lo)
    .bind(hi)
    .bind(path)
    .bind(q.user_id)
    .bind(limit)
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?;
    let items: Vec<RequestRow> = rows
        .iter()
        .map(|r| RequestRow {
            id: r.get("id"),
            ts: r.get::<chrono::DateTime<chrono::Utc>, _>("ts").to_rfc3339(),
            method: r.get("method"),
            path: r.get("path"),
            status: r.get::<i32, _>("status") as i64,
            latency_ms: r.get::<i32, _>("latency_ms") as i64,
            user_id: r.get::<Option<Uuid>, _>("user_id").map(|u| u.to_string()),
            ip: r.get("ip"),
            user_agent: r.get("user_agent"),
            detail: r.get("detail"),
        })
        .collect();
    let next = if items.len() as i64 == limit {
        items.last().map(|r| r.id)
    } else {
        None
    };
    Ok(json!({ "items": items, "nextBeforeId": next }))
}

/// `GET /api/workspaces/{ws}/inspector/stats` — last 24 hours.
pub async fn stats(app: &AeroApp, ws: Uuid) -> AeroResult<Value> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS total,
                COUNT(*) FILTER (WHERE status >= 400) AS errors,
                COUNT(*) FILTER (WHERE path LIKE '%/ai/%') AS ai,
                COALESCE(AVG(latency_ms), 0)::float8 AS avg_latency,
                COUNT(DISTINCT user_id) AS active_users
         FROM requests WHERE workspace_id = $1 AND ts >= now() - interval '24 hours'",
    )
    .bind(ws)
    .fetch_one(app.db.pool())
    .await
    .map_err(db_err)?;
    let hourly: Vec<Value> = sqlx::query(
        "SELECT to_char(date_trunc('hour', ts) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:00') AS hour,
                COUNT(*) AS total, COUNT(*) FILTER (WHERE status >= 400) AS errors
         FROM requests WHERE workspace_id = $1 AND ts >= now() - interval '24 hours'
         GROUP BY 1 ORDER BY 1",
    )
    .bind(ws)
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?
    .iter()
    .map(|r| {
        json!({
            "hour": r.get::<String, _>("hour"),
            "total": r.get::<i64, _>("total"),
            "errors": r.get::<i64, _>("errors"),
        })
    })
    .collect();
    let members: i64 = sqlx::query("SELECT COUNT(*) AS n FROM memberships WHERE workspace_id = $1")
        .bind(ws)
        .fetch_one(app.db.pool())
        .await
        .map_err(db_err)?
        .get("n");
    Ok(json!({
        "requests24h": row.get::<i64, _>("total"),
        "errors24h": row.get::<i64, _>("errors"),
        "aiRequests24h": row.get::<i64, _>("ai"),
        "avgLatencyMs": row.get::<f64, _>("avg_latency").round() as i64,
        "activeUsers24h": row.get::<i64, _>("active_users"),
        "members": members,
        "hourly": hourly,
    }))
}

// ---------------------------------------------------------------- /v1 shapes

fn trace_from_row(r: &sqlx::postgres::PgRow) -> RequestTrace {
    RequestTrace {
        id: Uuid::from_u64_pair(0, r.get::<i64, _>("id") as u64),
        method: r.get("method"),
        path: r.get("path"),
        status: r.get::<i32, _>("status") as u16,
        duration_ms: r.get::<i32, _>("latency_ms") as f64,
        request_headers: Default::default(),
        response_headers: Default::default(),
        request_body_size: 0,
        response_body_size: 0,
        user_id: r.get("user_id"),
        workspace_id: r.get("workspace_id"),
        client_ip: r.get::<Option<String>, _>("ip").unwrap_or_default(),
        user_agent: r.get("user_agent"),
        timestamp: r.get("ts"),
        error: None,
    }
}

/// `GET /v1/inspector/traces` — newest first.
pub async fn traces(app: &AeroApp, ws: Uuid, q: &InspectorQuery) -> AeroResult<Vec<RequestTrace>> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500) as i64;
    let prefix = q
        .path_prefix
        .as_deref()
        .map(|p| format!("{}%", p.replace(['%', '_'], "")));
    let rows = sqlx::query(
        "SELECT id, ts, method, path, status, latency_ms, user_id, workspace_id, ip, user_agent FROM requests
         WHERE workspace_id = $1 AND ($2::text IS NULL OR method = $2) AND ($3::text IS NULL OR path LIKE $3)
           AND status >= $4 AND status <= $5 AND latency_ms >= $6 AND ($7::uuid IS NULL OR user_id = $7)
         ORDER BY id DESC LIMIT $8",
    )
    .bind(ws)
    .bind(q.method.as_deref().map(str::to_ascii_uppercase))
    .bind(prefix)
    .bind(q.min_status.unwrap_or(0) as i32)
    .bind(q.max_status.unwrap_or(999) as i32)
    .bind(q.min_duration_ms.unwrap_or(0.0) as i32)
    .bind(q.user_id)
    .bind(limit)
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?;
    Ok(rows.iter().map(trace_from_row).collect())
}

/// `GET /v1/inspector/stats` — last 24 hours of one workspace.
pub async fn v1_stats(app: &AeroApp, ws: Uuid) -> AeroResult<InspectorStats> {
    let r = sqlx::query(
        "SELECT COUNT(*) AS total, COUNT(*) FILTER (WHERE status >= 400) AS errors,
                COALESCE(AVG(latency_ms), 0)::float8 AS avg,
                COALESCE(percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms), 0)::float8 AS p50,
                COALESCE(percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms), 0)::float8 AS p95,
                COALESCE(percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms), 0)::float8 AS p99,
                COUNT(*) FILTER (WHERE ts >= now() - interval '60 seconds') AS last_minute
         FROM requests WHERE workspace_id = $1 AND ts >= now() - interval '24 hours'",
    )
    .bind(ws)
    .fetch_one(app.db.pool())
    .await
    .map_err(db_err)?;
    let recent = traces(
        app,
        ws,
        &InspectorQuery {
            method: None,
            path_prefix: None,
            min_status: None,
            max_status: None,
            min_duration_ms: None,
            user_id: None,
            workspace_id: None,
            limit: Some(20),
        },
    )
    .await?;
    Ok(InspectorStats {
        total_requests: r.get::<i64, _>("total") as u64,
        total_errors: r.get::<i64, _>("errors") as u64,
        avg_latency_ms: r.get("avg"),
        p50_latency_ms: r.get("p50"),
        p95_latency_ms: r.get("p95"),
        p99_latency_ms: r.get("p99"),
        rps: r.get::<i64, _>("last_minute") as f64 / 60.0,
        active_connections: 0,
        traces: recent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_invite_tokens() {
        assert_eq!(
            sanitize_path("/api/invites/abcDEF/accept"),
            "/api/invites/:token/accept"
        );
        assert_eq!(sanitize_path("/api/invites/abc"), "/api/invites/:token");
        assert_eq!(
            sanitize_path("/api/workspaces/w/invites"),
            "/api/workspaces/w/invites"
        );
        let id = Uuid::new_v4();
        assert_eq!(
            workspace_from_path(&format!("/api/workspaces/{id}/members")),
            Some(id)
        );
        assert_eq!(workspace_from_path("/v1/workspaces/invite/accept"), None);
    }
}
