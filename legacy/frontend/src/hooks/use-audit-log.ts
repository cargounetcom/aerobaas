import { useState, useEffect, useCallback } from "react";
import { api } from "@/lib/api";
import { useWebSocket } from "./use-websocket";

export interface AuditEvent {
  id: string;
  workspace_id: string;
  actor_type: "user" | "ai_agent" | "system" | "hazmat_module" | "remediation_module" | "erlang_orchestrator";
  actor_id: string | null;
  actor_name: string;
  action: string;
  resource_type: string;
  resource_id: string | null;
  details: Record<string, any>;
  severity: "info" | "warning" | "critical" | "security";
  ip_address: string | null;
  user_agent: string | null;
  timestamp: string;
}

export function useAuditLog(workspaceId?: string) {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<{
    actor_type?: string;
    action?: string;
    severity?: string;
  }>({});

  const loadInitial = useCallback(async () => {
    try {
      const params: Record<string, string> = { limit: "100" };
      if (workspaceId) params.workspace_id = workspaceId;
      if (filter.actor_type) params.actor_type = filter.actor_type;
      if (filter.action) params.action = filter.action;
      if (filter.severity) params.severity = filter.severity;

      const resp = await api.getAuditEvents(params);
      setEvents(resp.events);
    } catch (err) {
      console.error("Failed to load audit events:", err);
    } finally {
      setLoading(false);
    }
  }, [workspaceId, filter]);

  useEffect(() => {
    loadInitial();
  }, [loadInitial]);

  const { isConnected } = useWebSocket({
    path: "/v1/audit/stream",
    onMessage: (data) => {
      const event = data.event || data;
      setEvents((prev) => [event, ...prev].slice(0, 500));
    },
  });

  return { events, loading, filter, setFilter, isConnected, reload: loadInitial };
}
