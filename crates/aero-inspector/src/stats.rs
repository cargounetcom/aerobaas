use aero_protocol::inspector::*;
use crate::collector::TraceCollector;

pub struct StatsAggregator;

impl StatsAggregator {
    pub fn aggregate(collector: &TraceCollector, query: Option<&InspectorQuery>) -> InspectorStats {
        let traces = match query {
            Some(q) => collector.query(q),
            None => collector.query(&InspectorQuery {
                method: None,
                path_prefix: None,
                min_status: None,
                max_status: None,
                min_duration_ms: None,
                user_id: None,
                workspace_id: None,
                limit: Some(1000),
            }),
        };

        let total = traces.len();
        let errors = traces.iter().filter(|t| t.status >= 400).count();

        let mut latencies: Vec<f64> = traces.iter().map(|t| t.duration_ms).collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg = if total > 0 {
            latencies.iter().sum::<f64>() / total as f64
        } else {
            0.0
        };

        let p50 = percentile(&latencies, 0.50);
        let p95 = percentile(&latencies, 0.95);
        let p99 = percentile(&latencies, 0.99);

        let rps = if total > 0 {
            let earliest = traces.iter().map(|t| t.timestamp).min();
            let latest = traces.iter().map(|t| t.timestamp).max();
            match (earliest, latest) {
                (Some(e), Some(l)) => {
                    let span = (l - e).num_seconds().max(1) as f64;
                    total as f64 / span
                }
                _ => 0.0,
            }
        } else {
            0.0
        };

        InspectorStats {
            total_requests: collector.total_requests(),
            total_errors: collector.total_errors(),
            avg_latency_ms: avg,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            rps,
            active_connections: 0,
            traces: traces.into_iter().take(100).collect(),
        }
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p * sorted.len() as f64) - 1.0).max(0.0) as usize;
    sorted[idx.min(sorted.len() - 1)]
}
