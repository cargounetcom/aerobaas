use std::sync::Arc;

use axum::{routing::{get, post, put, delete}, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::app::AeroApp;
use crate::error::AeroApiError;

pub fn build_router(app: &Arc<AeroApp>) -> Router {
    let state = AppState {
        app: app.clone(),
    };

    Router::new()
        // Health
        .route("/health", get(health))

        // Auth
        .route("/v1/auth/register", post(auth_register))
        .route("/v1/auth/login", post(auth_login))
        .route("/v1/auth/refresh", post(auth_refresh))
        .route("/v1/auth/me", get(auth_me))
        .route("/v1/auth/switch-workspace", post(auth_switch_workspace))

        // Workspaces (Tenancy)
        .route("/v1/workspaces", post(create_workspace).get(list_workspaces))
        .route("/v1/workspaces/:id", get(get_workspace).put(update_workspace).delete(delete_workspace))
        .route("/v1/workspaces/:id/members", get(list_members))
        .route("/v1/workspaces/:id/members/:user_id", put(update_member_role).delete(remove_member))
        .route("/v1/workspaces/:id/invite", post(invite_member))
        .route("/v1/workspaces/invite/accept", post(accept_invite))

        // Keys API 2.0
        .route("/v1/keys", post(create_key).get(list_keys))
        .route("/v1/keys/:id", get(get_key).delete(delete_key))
        .route("/v1/keys/verify", post(verify_key))

        // Functions
        .route("/v1/functions", post(deploy_function).get(list_functions))
        .route("/v1/functions/:id", get(get_function).delete(delete_function))
        .route("/v1/functions/:id/execute", post(execute_function))

        // Remediation
        .route("/v1/remediation/scan", post(scan_dependencies))
        .route("/v1/remediation/remediate", post(remediate))

        // Skills
        .route("/v1/skills", get(list_skills).post(install_skill))
        .route("/v1/skills/:id", get(get_skill))
        .route("/v1/deploy/auto-detect", post(deploy_auto_detect))

        // MCP
        .route("/v1/mcp", get(mcp_ws_handler))

        // AI
        .route("/v1/ai/generate", post(ai_generate))

        // Chat
        .route("/v1/chat/conversations", post(create_conversation).get(list_conversations))
        .route("/v1/chat/conversations/:id", get(get_conversation).delete(delete_conversation))
        .route("/v1/chat/conversations/:id/messages", get(get_messages))
        .route("/v1/chat/conversations/:id/send", post(send_message))
        .route("/v1/chat/conversations/:id/panel-config", put(update_panel_config))
        .route("/v1/chat/panels", get(list_panels))
        .route("/v1/chat/comfort", get(get_comfort_panel))

        // Inspector
        .route("/v1/inspector/stats", get(inspector_stats))
        .route("/v1/inspector/traces", get(inspector_traces))

        // Audit Log
        .route("/v1/audit/events", get(audit_events))
        .route("/v1/audit/recent", get(audit_recent))
        .route("/v1/audit/stream", get(audit_stream))

        // Metrics
        .route("/v1/metrics/snapshot", get(metrics_snapshot))
        .route("/v1/metrics/sandboxes", get(metrics_sandboxes))
        .route("/v1/metrics/sandboxes/:id", get(metrics_sandbox_detail))
        .route("/v1/metrics/history", get(metrics_history))

        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Clone)]
pub struct AppState {
    pub app: Arc<AeroApp>,
}

async fn health() -> &'static str {
    "AeroBaaS OK"
}

// === Auth ===

async fn auth_register(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::auth::RegisterRequest>,
) -> Result<Json<aero_protocol::auth::AuthResponse>, AeroApiError> {
    let resp = state.app.auth.register(req).await?;
    Ok(Json(resp))
}

async fn auth_login(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::auth::LoginRequest>,
) -> Result<Json<aero_protocol::auth::AuthResponse>, AeroApiError> {
    let resp = state.app.auth.login(req).await?;
    Ok(Json(resp))
}

async fn auth_refresh(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::auth::RefreshTokenRequest>,
) -> Result<Json<aero_protocol::auth::AuthResponse>, AeroApiError> {
    let resp = state.app.auth.refresh(req).await?;
    Ok(Json(resp))
}

async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<aero_protocol::auth::UserPublic>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let user = state.app.auth.get_user(claims.sub).await?;
    Ok(Json(user))
}

async fn auth_switch_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<aero_protocol::auth::SwitchWorkspaceRequest>,
) -> Result<Json<aero_protocol::auth::SwitchWorkspaceResponse>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let resp = state.app.auth.switch_workspace(claims.sub, req).await?;
    Ok(Json(resp))
}

// === Workspaces ===

async fn create_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<aero_protocol::tenancy::CreateWorkspaceRequest>,
) -> Result<Json<aero_protocol::tenancy::Workspace>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let ws = state.app.tenancy.create_workspace(claims.sub, req).await?;
    Ok(Json(ws))
}

async fn list_workspaces(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<aero_protocol::tenancy::Workspace>>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let workspaces = state.app.tenancy.list_workspaces(claims.sub).await?;
    Ok(Json(workspaces))
}

async fn get_workspace(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<aero_protocol::tenancy::Workspace>, AeroApiError> {
    let ws = state.app.tenancy.get_workspace(id).await?;
    Ok(Json(ws))
}

async fn update_workspace(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<aero_protocol::tenancy::UpdateWorkspaceRequest>,
) -> Result<Json<aero_protocol::tenancy::Workspace>, AeroApiError> {
    let ws = state.app.tenancy.update_workspace(id, req).await?;
    Ok(Json(ws))
}

async fn delete_workspace(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AeroApiError> {
    state.app.tenancy.delete_workspace(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(state): State<AppState>,
    Path(workspace_id): Path<uuid::Uuid>,
) -> Result<Json<aero_protocol::tenancy::ListMembersResponse>, AeroApiError> {
    let resp = state.app.tenancy.list_members(workspace_id).await?;
    Ok(Json(resp))
}

async fn update_member_role(
    State(state): State<AppState>,
    Path((workspace_id, user_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> Result<StatusCode, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let role_str = req.get("role").and_then(|v| v.as_str()).unwrap_or("user");
    let role = match role_str {
        "owner" => aero_protocol::tenancy::Role::Owner,
        "admin" => aero_protocol::tenancy::Role::Admin,
        "manager" => aero_protocol::tenancy::Role::Manager,
        _ => aero_protocol::tenancy::Role::User,
    };
    state.app.tenancy.update_member_role(workspace_id, claims.sub, user_id, role).await?;
    Ok(StatusCode::OK)
}

async fn remove_member(
    State(state): State<AppState>,
    Path((workspace_id, user_id)): Path<(uuid::Uuid, uuid::Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    state.app.tenancy.remove_member(workspace_id, claims.sub, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn invite_member(
    State(state): State<AppState>,
    Path(workspace_id): Path<uuid::Uuid>,
    headers: HeaderMap,
    Json(req): Json<aero_protocol::tenancy::InviteRequest>,
) -> Result<Json<aero_protocol::tenancy::InviteResponse>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let resp = state.app.tenancy.invite_member(workspace_id, claims.sub, req).await?;
    Ok(Json(resp))
}

async fn accept_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<aero_protocol::tenancy::AcceptInviteRequest>,
) -> Result<Json<aero_protocol::tenancy::Membership>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let membership = state.app.tenancy.accept_invite(&req.invite_token, claims.sub).await?;
    Ok(Json(membership))
}

// === Keys ===

async fn create_key(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::keys::CreateKeyRequest>,
) -> Result<Json<aero_protocol::keys::CreateKeyResponse>, AeroApiError> {
    let resp = state.app.key_manager.create_key(req).await?;
    Ok(Json(resp))
}

async fn list_keys(
    State(state): State<AppState>,
) -> Result<Json<Vec<aero_protocol::keys::ApiKey>>, AeroApiError> {
    let keys = state.app.key_manager.list_keys().await?;
    Ok(Json(keys))
}

async fn get_key(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<aero_protocol::keys::ApiKey>, AeroApiError> {
    let key = state.app.key_manager.get_key(id).await?;
    Ok(Json(key))
}

async fn delete_key(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AeroApiError> {
    state.app.key_manager.delete_key(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn verify_key(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::keys::VerifyKeyRequest>,
) -> Result<Json<aero_protocol::keys::VerifyKeyResponse>, AeroApiError> {
    let resp = state.app.key_manager.verify_key(req).await?;
    Ok(Json(resp))
}

// === Functions ===

async fn deploy_function(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::functions::DeployRequest>,
) -> Result<Json<aero_protocol::functions::DeployResponse>, AeroApiError> {
    let resp = state.app.wasm_runtime.deploy(req, &state.app.remediation).await?;
    Ok(Json(resp))
}

async fn list_functions(
    State(state): State<AppState>,
) -> Result<Json<Vec<aero_protocol::functions::Function>>, AeroApiError> {
    let funcs = state.app.db.list_functions().await?;
    Ok(Json(funcs))
}

async fn get_function(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<aero_protocol::functions::Function>, AeroApiError> {
    let func = state.app.db.get_function(id).await?;
    Ok(Json(func))
}

async fn delete_function(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AeroApiError> {
    state.app.db.delete_function(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn execute_function(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<aero_protocol::functions::ExecuteRequest>,
) -> Result<Json<aero_protocol::functions::ExecuteResponse>, AeroApiError> {
    let resp = state.app.wasm_runtime.execute(id, req.payload).await?;
    Ok(Json(resp))
}

// === Remediation ===

async fn scan_dependencies(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::remediation::ScanRequest>,
) -> Result<Json<aero_protocol::remediation::ScanResult>, AeroApiError> {
    let result = state.app.remediation.scan(req).await?;
    Ok(Json(result))
}

async fn remediate(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::remediation::ScanRequest>,
) -> Result<Json<aero_protocol::remediation::RemediationResult>, AeroApiError> {
    let result = state.app.remediation.remediate(req).await?;
    Ok(Json(result))
}

// === Skills ===

async fn list_skills(
    State(state): State<AppState>,
) -> Result<Json<Vec<aero_protocol::skills::SkillPack>>, AeroApiError> {
    let skills = state.app.skill_registry.list_skills().await?;
    Ok(Json(skills))
}

async fn get_skill(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<aero_protocol::skills::SkillPack>, AeroApiError> {
    let skill = state.app.skill_registry.get_skill(&id).await?;
    Ok(Json(skill))
}

async fn install_skill(
    State(state): State<AppState>,
    Json(skill): Json<aero_protocol::skills::SkillPack>,
) -> Result<Json<aero_protocol::skills::SkillPack>, AeroApiError> {
    let skill = state.app.skill_registry.install_skill(skill).await?;
    Ok(Json(skill))
}

async fn deploy_auto_detect(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::skills::DeployAutoDetectRequest>,
) -> Result<Json<aero_protocol::skills::DeployAutoDetectResponse>, AeroApiError> {
    let resp = state.app.skill_registry.deploy_auto_detect(req, &state.app.wasm_runtime, &state.app.ai_engine).await?;
    Ok(Json(resp))
}

// === MCP ===

async fn mcp_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| state.app.mcp_server.handle_connection(socket))
}

// === AI ===

async fn ai_generate(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AeroApiError> {
    let prompt = req.get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AeroApiError::BadRequest("Missing 'prompt' field".into()))?;
    let result = state.app.ai_engine.generate_text(prompt).await?;
    Ok(Json(serde_json::json!({ "text": result })))
}

// === Chat ===

async fn create_conversation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<aero_protocol::chat::CreateConversationRequest>,
) -> Result<Json<aero_protocol::chat::Conversation>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let ws_id = claims.workspace_id.unwrap_or_default();
    let conv = state.app.chat.create_conversation(ws_id, claims.sub, req).await?;
    Ok(Json(conv))
}

async fn list_conversations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<aero_protocol::chat::ConversationSummary>>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let convs = state.app.chat.list_conversations(claims.sub).await?;
    Ok(Json(convs))
}

async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<aero_protocol::chat::Conversation>, AeroApiError> {
    let conv = state.app.chat.get_conversation(id).await?;
    Ok(Json(conv))
}

async fn delete_conversation(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AeroApiError> {
    state.app.chat.delete_conversation(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_messages(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Vec<aero_protocol::chat::Message>>, AeroApiError> {
    let msgs = state.app.chat.get_messages(id).await?;
    Ok(Json(msgs))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    Json(mut req): Json<aero_protocol::chat::SendMessageRequest>,
) -> Result<Json<aero_protocol::chat::SendMessageResponse>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    req.conversation_id = id;
    let resp = state.app.chat.send_message(claims.sub, req).await?;
    Ok(Json(resp))
}

async fn update_panel_config(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<aero_protocol::chat::PanelConfig>,
) -> Result<Json<aero_protocol::chat::Conversation>, AeroApiError> {
    let conv = state.app.chat.update_panel_config(id, req).await?;
    Ok(Json(conv))
}

async fn list_panels() -> Result<Json<Vec<aero_protocol::chat::Panel>>, AeroApiError> {
    Ok(Json(aero_chat::PanelManager::default_panels()))
}

async fn get_comfort_panel(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<aero_protocol::chat::ComfortPanel>, AeroApiError> {
    let token = extract_bearer(&headers)?;
    let claims = state.app.auth.verify_token(&token)?;
    let user = state.app.auth.get_user(claims.sub).await?;
    let conversations = state.app.chat.list_conversations(claims.sub).await?;
    let stats = aero_protocol::chat::ComfortStats {
        total_conversations: conversations.len() as u32,
        total_tokens: 0,
        total_deployments: 0,
        avg_latency_ms: 0.0,
        uptime_hours: 0.0,
    };
    let panel = aero_chat::PanelManager::build_comfort_panel(
        Some(&user.name),
        &conversations,
        stats,
    );
    Ok(Json(panel))
}

// === Inspector ===

async fn inspector_stats(
    State(state): State<AppState>,
) -> Result<Json<aero_protocol::inspector::InspectorStats>, AeroApiError> {
    let stats = aero_inspector::StatsAggregator::aggregate(&state.app.inspector, None);
    Ok(Json(stats))
}

async fn inspector_traces(
    State(state): State<AppState>,
    Query(q): Query<aero_protocol::inspector::InspectorQuery>,
) -> Result<Json<Vec<aero_protocol::inspector::RequestTrace>>, AeroApiError> {
    let traces = state.app.inspector.query(&q);
    Ok(Json(traces))
}

// === Audit Log ===

async fn audit_events(
    State(state): State<AppState>,
    Query(q): Query<aero_protocol::audit::AuditLogQuery>,
) -> Result<Json<aero_protocol::audit::AuditLogResponse>, AeroApiError> {
    let resp = state.app.audit_logger.query(&q);
    Ok(Json(resp))
}

async fn audit_recent(
    State(state): State<AppState>,
    Query(q): Query<serde_json::Value>,
) -> Result<Json<Vec<aero_protocol::audit::AuditEvent>>, AeroApiError> {
    let limit = q.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let events = state.app.audit_logger.recent(limit);
    Ok(Json(events))
}

async fn audit_stream(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let mut rx = state.app.event_broadcaster.subscribe();
        tracing::info!("Audit stream WebSocket connected");

        while let Ok(event) = rx.recv().await {
            let msg = serde_json::to_string(&event).unwrap_or_default();
            if socket.send(axum::extract::ws::Message::Text(msg)).await.is_err() {
                break;
            }
        }

        state.app.event_broadcaster.unsubscribe();
        tracing::info!("Audit stream WebSocket disconnected");
    })
}

// === Metrics ===

async fn metrics_snapshot(
    State(state): State<AppState>,
) -> Result<Json<aero_protocol::metrics::MetricsSnapshot>, AeroApiError> {
    let snapshot = state.app.metrics_collector.snapshot();
    Ok(Json(snapshot))
}

async fn metrics_sandboxes(
    State(state): State<AppState>,
) -> Result<Json<Vec<aero_protocol::metrics::SandboxMetrics>>, AeroApiError> {
    let sandboxes = state.app.metrics_collector.list_sandboxes();
    Ok(Json(sandboxes))
}

async fn metrics_sandbox_detail(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<aero_protocol::metrics::SandboxMetrics, AeroApiError> {
    let metrics = state.app.metrics_collector.get_sandbox(id)
        .ok_or_else(|| AeroApiError::BadRequest("Sandbox not found".into()))?;
    Ok(Json(metrics))
}

async fn metrics_history(
    State(state): State<AppState>,
    Query(q): Query<aero_protocol::metrics::MetricsQuery>,
) -> Result<Json<aero_protocol::metrics::MetricsHistory, AeroApiError> {
    let window = q.window_minutes.unwrap_or(60);
    let history = state.app.metrics_history.get_history(window);
    Ok(Json(history))
}

// === Helpers ===

fn extract_bearer(headers: &HeaderMap) -> Result<String, AeroApiError> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .ok_or_else(|| AeroApiError::BadRequest("Missing Authorization header".into()))
}

use axum::extract::{Path, State, WebSocketUpgrade, Query};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
