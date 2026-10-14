use std::collections::VecDeque;

use aero_protocol::metrics::*;
use parking_lot::RwLock;

pub struct MetricsHistoryStore {
    cold_start_history: RwLock<VecDeque<ColdStartHistoryPoint>>,
    memory_history: RwLock<VecDeque<MemoryThrottlingPoint>>,
    max_points: usize,
}

impl MetricsHistoryStore {
    pub fn new(max_points: usize) -> Self {
        Self {
            cold_start_history: RwLock::new(VecDeque::with_capacity(max_points)),
            memory_history: RwLock::new(VecDeque::with_capacity(max_points)),
            max_points,
        }
    }

    pub fn record_cold_start(&self, point: ColdStartHistoryPoint) {
        let mut history = self.cold_start_history.write();
        if history.len() >= self.max_points {
            history.pop_front();
        }
        history.push_back(point);
    }

    pub fn record_memory(&self, point: MemoryThrottlingPoint) {
        let mut history = self.memory_history.write();
        if history.len() >= self.max_points {
            history.pop_front();
        }
        history.push_back(point);
    }

    pub fn get_history(&self, window_minutes: u32) -> MetricsHistory {
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(window_minutes as i64);

        let cold_start: Vec<ColdStartHistoryPoint> = self
            .cold_start_history
            .read()
            .iter()
            .filter(|p| p.timestamp > cutoff)
            .cloned()
            .collect();

        let memory: Vec<MemoryThrottlingPoint> = self
            .memory_history
            .read()
            .iter()
            .filter(|p| p.timestamp > cutoff)
            .cloned()
            .collect();

        MetricsHistory {
            cold_start_history: cold_start,
            memory_history: memory,
            window_minutes,
        }
    }

    pub fn cold_start_points(&self, limit: usize) -> Vec<ColdStartHistoryPoint> {
        self.cold_start_history
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn memory_points(&self, limit: usize) -> Vec<MemoryThrottlingPoint> {
        self.memory_history
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}
