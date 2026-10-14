use std::collections::VecDeque;

use aero_protocol::inspector::*;
use parking_lot::RwLock;

pub struct TraceCollector {
    config: InspectorConfig,
    traces: RwLock<VecDeque<RequestTrace>>,
    total_requests: RwLock<u64>,
    total_errors: RwLock<u64>,
}

impl TraceCollector {
    pub fn new(config: InspectorConfig) -> Self {
        let capacity = config.max_traces;
        Self {
            config,
            traces: RwLock::new(VecDeque::with_capacity(capacity)),
            total_requests: RwLock::new(0),
            total_errors: RwLock::new(0),
        }
    }

    pub fn record(&self, trace: RequestTrace) {
        if !self.config.enabled {
            return;
        }

        let mut traces = self.traces.write();
        if traces.len() >= self.config.max_traces {
            traces.pop_front();
        }
        traces.push_back(trace.clone());

        *self.total_requests.write() += 1;
        if trace.status >= 400 {
            *self.total_errors.write() += 1;
        }

        if trace.duration_ms > self.config.slow_request_threshold_ms {
            tracing::warn!(
                path = %trace.path,
                duration_ms = trace.duration_ms,
                "Slow request detected"
            );
        }
    }

    pub fn query(&self, q: &InspectorQuery) -> Vec<RequestTrace> {
        let traces = self.traces.read();
        let mut result: Vec<RequestTrace> = traces
            .iter()
            .filter(|t| {
                if let Some(ref method) = q.method {
                    if &t.method != method {
                        return false;
                    }
                }
                if let Some(ref prefix) = q.path_prefix {
                    if !t.path.starts_with(prefix) {
                        return false;
                    }
                }
                if let Some(min) = q.min_status {
                    if t.status < min {
                        return false;
                    }
                }
                if let Some(max) = q.max_status {
                    if t.status > max {
                        return false;
                    }
                }
                if let Some(min_dur) = q.min_duration_ms {
                    if t.duration_ms < min_dur {
                        return false;
                    }
                }
                if let Some(uid) = q.user_id {
                    if t.user_id != Some(uid) {
                        return false;
                    }
                }
                if let Some(wsid) = q.workspace_id {
                    if t.workspace_id != Some(wsid) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let limit = q.limit.unwrap_or(100);
        result.truncate(limit);
        result
    }

    pub fn total_requests(&self) -> u64 {
        *self.total_requests.read()
    }

    pub fn total_errors(&self) -> u64 {
        *self.total_errors.read()
    }

    pub fn trace_count(&self) -> usize {
        self.traces.read().len()
    }
}
