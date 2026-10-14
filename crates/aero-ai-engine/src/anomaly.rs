use std::collections::VecDeque;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct AnomalyDetector {
    request_history: RwLock<VecDeque<RequestSnapshot>>,
    window_size: usize,
    ddos_threshold: u32,
    loop_detection_threshold: Duration,
}

struct RequestSnapshot {
    timestamp: Instant,
    function_id: Uuid,
    duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct AnomalyAlert {
    pub alert_type: AlertType,
    pub function_id: Uuid,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertType {
    DdosAttack,
    InfiniteLoop,
    ResourceExhaustion,
    SuspiciousTraffic,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self {
            request_history: RwLock::new(VecDeque::with_capacity(10_000)),
            window_size: 1000,
            ddos_threshold: 5000,
            loop_detection_threshold: Duration::from_secs(30),
        }
    }

    pub fn record_request(&self, function_id: Uuid, duration_ms: u64) {
        let mut history = self.request_history.write();
        history.push_back(RequestSnapshot {
            timestamp: Instant::now(),
            function_id,
            duration_ms,
        });
        if history.len() > self.window_size {
            history.pop_front();
        }
    }

    pub fn detect_anomalies(&self) -> Vec<AnomalyAlert> {
        let history = self.request_history.read();
        let now = Instant::now();
        let window = Duration::from_secs(1);

        let recent_count = history
            .iter()
            .filter(|s| now.duration_since(s.timestamp) < window)
            .count() as u32;

        let mut alerts = Vec::new();

        if recent_count > self.ddos_threshold {
            alerts.push(AnomalyAlert {
                alert_type: AlertType::DdosAttack,
                function_id: Uuid::nil(),
                details: format!(
                    "RPS {recent_count} exceeds threshold {}",
                    self.ddos_threshold
                ),
            });
        }

        for snapshot in history.iter().rev().take(100) {
            if snapshot.duration_ms > self.loop_detection_threshold.as_millis() as u64 {
                alerts.push(AnomalyAlert {
                    alert_type: AlertType::InfiniteLoop,
                    function_id: snapshot.function_id,
                    details: format!(
                        "Execution took {}ms — possible infinite loop",
                        snapshot.duration_ms
                    ),
                });
                break;
            }
        }

        alerts
    }
}
