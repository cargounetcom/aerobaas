use std::sync::Arc;

use aero_protocol::audit::{AuditEvent, RealtimeEvent};
use parking_lot::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

pub struct EventBroadcaster {
    tx: broadcast::Sender<RealtimeEvent>,
    subscriber_count: RwLock<usize>,
}

impl EventBroadcaster {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, _) = broadcast::channel(buffer_size);
        Self {
            tx,
            subscriber_count: RwLock::new(0),
        }
    }

    pub fn broadcast(&self, event: AuditEvent) {
        let realtime = RealtimeEvent {
            event: event.clone(),
            stream_id: format!("evt_{}", event.id.simple()),
        };
        let _ = self.tx.send(realtime);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeEvent> {
        *self.subscriber_count.write() += 1;
        tracing::info!(subscribers = *self.subscriber_count.read(), "New WebSocket subscriber for audit events");
        self.tx.subscribe()
    }

    pub fn unsubscribe(&self) {
        let mut count = self.subscriber_count.write();
        if *count > 0 {
            *count -= 1;
        }
        tracing::info!(subscribers = *count, "WebSocket subscriber disconnected");
    }

    pub fn subscriber_count(&self) -> usize {
        *self.subscriber_count.read()
    }
}

pub struct BroadcastHandle {
    pub rx: broadcast::Receiver<RealtimeEvent>,
    pub broadcaster: Arc<EventBroadcaster>,
}

impl Drop for BroadcastHandle {
    fn drop(&mut self) {
        self.broadcaster.unsubscribe();
    }
}
