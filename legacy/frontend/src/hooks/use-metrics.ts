import { useState, useEffect, useCallback, useRef } from "react";
import { api } from "@/lib/api";

export interface SandboxMetrics {
  function_id: string;
  function_name: string;
  instance_id: string;
  status: "idle" | "running" | "cold_start" | "quarantine" | "blocked" | "crashed";
  cold_start_ms: number;
  execution_duration_ms: number;
  memory_used_mb: number;
  memory_limit_mb: number;
  cpu_percent: number;
  cpu_limit_percent: number;
  network_egress_bytes: number;
  network_ingress_bytes: number;
  egress_blocked: number;
  invocations_total: number;
  errors_total: number;
  last_invocation: string | null;
  timestamp: string;
}

export interface AggregateMetrics {
  total_sandboxes: number;
  active_sandboxes: number;
  idle_sandboxes: number;
  blocked_sandboxes: number;
  total_invocations: number;
  total_errors: number;
  avg_cold_start_ms: number;
  p50_cold_start_ms: number;
  p95_cold_start_ms: number;
  p99_cold_start_ms: number;
  avg_memory_mb: number;
  max_memory_mb: number;
  avg_cpu_percent: number;
  total_egress_bytes: number;
  total_egress_blocked: number;
  rps: number;
}

export interface MetricsHistory {
  cold_start_history: Array<{
    timestamp: string;
    function_id: string;
    function_name: string;
    cold_start_ms: number;
  }>;
  memory_history: Array<{
    timestamp: string;
    function_id: string;
    function_name: string;
    memory_used_mb: number;
    memory_limit_mb: number;
    throttled: boolean;
  }>;
  window_minutes: number;
}

export function useMetrics(refreshInterval = 5000) {
  const [snapshot, setSnapshot] = useState<{
    sandboxes: SandboxMetrics[];
    aggregate: AggregateMetrics;
  } | null>(null);
  const [history, setHistory] = useState<MetricsHistory | null>(null);
  const [loading, setLoading] = useState(true);
  const timerRef = useRef<ReturnType<typeof setInterval>>();

  const refresh = useCallback(async () => {
    try {
      const [snap, hist] = await Promise.all([
        api.getMetricsSnapshot(),
        api.getMetricsHistory(60),
      ]);
      setSnapshot(snap);
      setHistory(hist);
    } catch (err) {
      console.error("Failed to load metrics:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    timerRef.current = setInterval(refresh, refreshInterval);
    return () => clearInterval(timerRef.current);
  }, [refresh, refreshInterval]);

  return { snapshot, history, loading, refresh };
}
