const API_BASE = import.meta.env.VITE_API_URL || "http://localhost:8080";
const WS_BASE = API_BASE.replace(/^http/, "ws");

export function getApiBase() {
  return API_BASE;
}

export function getWsBase() {
  return WS_BASE;
}

export function getAuthToken(): string | null {
  return localStorage.getItem("aero_token");
}

export function setAuthToken(token: string) {
  localStorage.setItem("aero_token", token);
}

export function getActiveWorkspaceId(): string | null {
  return localStorage.getItem("aero_workspace_id");
}

export function setActiveWorkspaceId(id: string) {
  localStorage.setItem("aero_workspace_id", id);
}

async function apiRequest<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = getAuthToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...((options.headers as Record<string, string>) || {}),
  };
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: "Request failed" }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }

  return response.json();
}

export const api = {
  // Auth
  register: (email: string, password: string, name: string) =>
    apiRequest<{ token: string; refresh_token: string; user: any }>("/v1/auth/register", {
      method: "POST",
      body: JSON.stringify({ email, password, name }),
    }),

  login: (email: string, password: string) =>
    apiRequest<{ token: string; refresh_token: string; user: any }>("/v1/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),

  me: () => apiRequest<{ id: string; email: string; name: string }>("/v1/auth/me"),

  // Workspaces
  listWorkspaces: () => apiRequest<any[]>("/v1/workspaces"),
  createWorkspace: (name: string, slug: string) =>
    apiRequest<any>("/v1/workspaces", { method: "POST", body: JSON.stringify({ name, slug }) }),
  inviteMember: (workspaceId: string, email: string, role: string) =>
    apiRequest<any>(`/v1/workspaces/${workspaceId}/invite`, {
      method: "POST",
      body: JSON.stringify({ email, role }),
    }),
  listMembers: (workspaceId: string) =>
    apiRequest<{ members: any[]; total: number }>(`/v1/workspaces/${workspaceId}/members`),

  // Keys
  createKey: (name: string, scopes: string[], credits?: number) =>
    apiRequest<{ id: string; key: string }>("/v1/keys", {
      method: "POST",
      body: JSON.stringify({ name, scopes, credits_limit: credits }),
    }),
  listKeys: () => apiRequest<any[]>("/v1/keys"),

  // Functions
  listFunctions: () => apiRequest<any[]>("/v1/functions"),
  executeFunction: (id: string, payload: any) =>
    apiRequest<any>(`/v1/functions/${id}/execute`, {
      method: "POST",
      body: JSON.stringify({ function_id: id, payload }),
    }),

  // Chat
  createConversation: (title?: string) =>
    apiRequest<any>("/v1/chat/conversations", {
      method: "POST",
      body: JSON.stringify({ title }),
    }),
  listConversations: () => apiRequest<any[]>("/v1/chat/conversations"),
  sendMessage: (conversationId: string, content: string) =>
    apiRequest<any>(`/v1/chat/conversations/${conversationId}/send`, {
      method: "POST",
      body: JSON.stringify({ conversation_id: conversationId, content }),
    }),
  getMessages: (conversationId: string) =>
    apiRequest<any[]>(`/v1/chat/conversations/${conversationId}/messages`),
  getComfortPanel: () => apiRequest<any>("/v1/chat/comfort"),

  // Audit
  getAuditEvents: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params)}` : "";
    return apiRequest<{ events: any[]; total: number; has_more: boolean }>(`/v1/audit/events${qs}`);
  },
  getRecentEvents: (limit = 50) =>
    apiRequest<any[]>(`/v1/audit/recent?limit=${limit}`),

  // Metrics
  getMetricsSnapshot: () => apiRequest<any>("/v1/metrics/snapshot"),
  getMetricsSandboxes: () => apiRequest<any[]>("/v1/metrics/sandboxes"),
  getMetricsHistory: (windowMinutes = 60) =>
    apiRequest<any>(`/v1/metrics/history?window_minutes=${windowMinutes}`),

  // Skills
  listSkills: () => apiRequest<any[]>("/v1/skills"),

  // Inspector
  getInspectorStats: () => apiRequest<any>("/v1/inspector/stats"),
  getInspectorTraces: () => apiRequest<any[]>("/v1/inspector/traces"),
};
