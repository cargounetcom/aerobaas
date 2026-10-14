//! Audit log: who did what. Stored in PostgreSQL and pushed to live subscribers.

use aero_db::{db_err, dec, enc};
use aero_protocol::audit::{
    ActorType, AuditAction, AuditEvent, AuditLogQuery, AuditLogResponse, EventSeverity,
};
use aero_protocol::AeroResult;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::app::AeroApp;
use crate::auth::{ClientInfo, Ctx};

/// The actor of an audit event.
pub struct Who {
    pub actor_type: ActorType,
    pub actor_id: Option<Uuid>,
    pub actor_name: String,
}

impl Who {
    pub fn user(id: Uuid, email: &str) -> Self {
        Self {
            actor_type: ActorType::User,
            actor_id: Some(id),
            actor_name: email.to_string(),
        }
    }

    pub fn anonymous(email: &str) -> Self {
        Self {
            actor_type: ActorType::User,
            actor_id: None,
            actor_name: email.to_string(),
        }
    }

    pub fn module(actor_type: ActorType, name: &str) -> Self {
        Self {
            actor_type,
            actor_id: None,
            actor_name: name.to_string(),
        }
    }
}

impl From<&Ctx> for Who {
    fn from(c: &Ctx) -> Self {
        Self {
            actor_type: c.actor_type(),
            actor_id: c.actor_id(),
            actor_name: c.actor_name(),
        }
    }
}

/// One audit entry to write.
pub struct Entry<'a> {
    pub workspace_id: Option<Uuid>,
    pub who: Who,
    pub action: AuditAction,
    pub resource_type: &'a str,
    pub resource_id: Option<String>,
    pub details: Value,
    pub severity: EventSeverity,
}

impl<'a> Entry<'a> {
    pub fn new(
        workspace_id: Option<Uuid>,
        who: Who,
        action: AuditAction,
        resource_type: &'a str,
    ) -> Self {
        Self {
            workspace_id,
            who,
            action,
            resource_type,
            resource_id: None,
            details: Value::Object(Default::default()),
            severity: EventSeverity::Info,
        }
    }
    pub fn id(mut self, id: impl ToString) -> Self {
        self.resource_id = Some(id.to_string());
        self
    }
    pub fn details(mut self, d: Value) -> Self {
        self.details = d;
        self
    }
    pub fn severity(mut self, s: EventSeverity) -> Self {
        self.severity = s;
        self
    }
}

/// Write an audit event. Failures are logged, never returned: auditing must not break the request.
pub async fn record(app: &AeroApp, client: &ClientInfo, e: Entry<'_>) {
    let event = AuditEvent {
        id: Uuid::new_v4(),
        workspace_id: e.workspace_id.unwrap_or_default(),
        actor_type: e.who.actor_type,
        actor_id: e.who.actor_id,
        actor_name: e.who.actor_name,
        action: e.action,
        resource_type: e.resource_type.to_string(),
        resource_id: e.resource_id,
        details: e.details,
        severity: e.severity,
        ip_address: client.ip.clone(),
        user_agent: client.user_agent.clone(),
        timestamp: chrono::Utc::now(),
    };
    let r = sqlx::query(
        "INSERT INTO audit_events (id, workspace_id, actor_type, actor_id, actor_name, action, resource_type,
            resource_id, details, severity, ip, user_agent, ts)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(event.id)
    .bind(e.workspace_id)
    .bind(enc(&event.actor_type))
    .bind(event.actor_id)
    .bind(&event.actor_name)
    .bind(enc(&event.action))
    .bind(&event.resource_type)
    .bind(&event.resource_id)
    .bind(&event.details)
    .bind(enc(&event.severity))
    .bind(&event.ip_address)
    .bind(&event.user_agent)
    .bind(event.timestamp)
    .execute(app.db.pool())
    .await;
    if let Err(err) = r {
        tracing::warn!(error = %err, "failed to write audit event");
    }
    if e.workspace_id.is_some() {
        app.event_broadcaster.broadcast(event);
    }
}

/// Events of one workspace, newest first.
pub async fn query(
    app: &AeroApp,
    workspace_id: Uuid,
    q: &AuditLogQuery,
) -> AeroResult<AuditLogResponse> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500) as i64;
    let offset = q.offset.unwrap_or(0).min(100_000) as i64;
    let action = q.action.as_ref().map(enc);
    let actor = q.actor_type.as_ref().map(enc);
    let severity = q.severity.as_ref().map(enc);
    let filter = "workspace_id = $1
        AND ($2::text IS NULL OR action = $2)
        AND ($3::text IS NULL OR actor_type = $3)
        AND ($4::text IS NULL OR severity = $4)
        AND ($5::timestamptz IS NULL OR ts >= $5)
        AND ($6::timestamptz IS NULL OR ts <= $6)";
    let total: i64 = sqlx::query(&format!(
        "SELECT COUNT(*) AS n FROM audit_events WHERE {filter}"
    ))
    .bind(workspace_id)
    .bind(&action)
    .bind(&actor)
    .bind(&severity)
    .bind(q.start_time)
    .bind(q.end_time)
    .fetch_one(app.db.pool())
    .await
    .map_err(db_err)?
    .get("n");
    let rows = sqlx::query(&format!(
        "SELECT id, workspace_id, actor_type, actor_id, actor_name, action, resource_type, resource_id, details,
                severity, ip, user_agent, ts
         FROM audit_events WHERE {filter} ORDER BY ts DESC, id LIMIT $7 OFFSET $8"
    ))
    .bind(workspace_id)
    .bind(&action)
    .bind(&actor)
    .bind(&severity)
    .bind(q.start_time)
    .bind(q.end_time)
    .bind(limit)
    .bind(offset)
    .fetch_all(app.db.pool())
    .await
    .map_err(db_err)?;
    let events = rows
        .iter()
        .map(|r| {
            Ok(AuditEvent {
                id: r.get("id"),
                workspace_id: r.get::<Option<Uuid>, _>("workspace_id").unwrap_or_default(),
                actor_type: dec(r.get::<&str, _>("actor_type"))?,
                actor_id: r.get("actor_id"),
                actor_name: r.get("actor_name"),
                action: dec(r.get::<&str, _>("action"))?,
                resource_type: r.get("resource_type"),
                resource_id: r.get("resource_id"),
                details: r.get("details"),
                severity: dec(r.get::<&str, _>("severity"))?,
                ip_address: r.get("ip"),
                user_agent: r.get("user_agent"),
                timestamp: r.get("ts"),
            })
        })
        .collect::<AeroResult<Vec<_>>>()?;
    Ok(AuditLogResponse {
        has_more: offset + (events.len() as i64) < total,
        total: total as usize,
        events,
    })
}
