use std::collections::VecDeque;
use std::sync::Arc;

use aero_protocol::audit::{AuditAction, AuditEvent, ActorType, AuditLogQuery, AuditLogResponse, EventSeverity};
use aero_protocol::AeroResult;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::broadcaster::EventBroadcaster;

pub struct AuditLogger {
    events: RwLock<VecDeque<AuditEvent>>,
    max_events: usize,
    broadcaster: Arc<EventBroadcaster>,
}

impl AuditLogger {
    pub fn new(broadcaster: Arc<EventBroadcaster>, max_events: usize) -> Self {
        Self {
            events: RwLock::new(VecDeque::with_capacity(max_events)),
            max_events,
            broadcaster,
        }
    }

    pub fn log(&self, event: AuditEvent) -> AeroResult<()> {
        tracing::info!(
            action = event.action.as_str(),
            actor = %event.actor_name,
            resource = %event.resource_type,
            severity = ?event.severity,
            "Audit event"
        );

        let mut events = self.events.write();
        if events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(event.clone());
        drop(events);

        self.broadcaster.broadcast(event);

        Ok(())
    }

    pub fn log_simple(
        &self,
        workspace_id: Uuid,
        actor_type: ActorType,
        actor_id: Option<Uuid>,
        actor_name: &str,
        action: AuditAction,
        resource_type: &str,
        resource_id: Option<String>,
        details: serde_json::Value,
        severity: EventSeverity,
    ) -> AeroResult<()> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            workspace_id,
            actor_type,
            actor_id,
            actor_name: actor_name.to_string(),
            action,
            resource_type: resource_type.to_string(),
            resource_id,
            details,
            severity,
            ip_address: None,
            user_agent: None,
            timestamp: chrono::Utc::now(),
        };
        self.log(event)
    }

    pub fn query(&self, q: &AuditLogQuery) -> AuditLogResponse {
        let events = self.events.read();
        let mut filtered: Vec<AuditEvent> = events
            .iter()
            .filter(|e| {
                if let Some(wsid) = q.workspace_id {
                    if e.workspace_id != wsid {
                        return false;
                    }
                }
                if let Some(ref actor_type) = q.actor_type {
                    if e.actor_type != *actor_type {
                        return false;
                    }
                }
                if let Some(ref action) = q.action {
                    if e.action != *action {
                        return false;
                    }
                }
                if let Some(ref severity) = q.severity {
                    if e.severity != *severity {
                        return false;
                    }
                }
                if let Some(start) = q.start_time {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = q.end_time {
                    if e.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let total = filtered.len();
        let offset = q.offset.unwrap_or(0);
        let limit = q.limit.unwrap_or(100);

        let has_more = total > offset + limit;
        filtered = filtered.into_iter().skip(offset).take(limit).collect();

        AuditLogResponse {
            events: filtered,
            total,
            has_more,
        }
    }

    pub fn recent(&self, limit: usize) -> Vec<AuditEvent> {
        let events = self.events.read();
        events.iter().rev().take(limit).cloned().collect()
    }

    pub fn count(&self) -> usize {
        self.events.read().len()
    }
}
