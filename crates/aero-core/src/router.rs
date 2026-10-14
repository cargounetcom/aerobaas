//! HTTP routes. Every route authenticates the caller and checks workspace membership/role or
//! API-key scope, except the explicitly public ones: `/health`, `/v1/auth/register`,
//! `/v1/auth/login`, `/v1/auth/refresh`, `/v1/keys/verify` and `GET /api/invites/{token}`.

use std::sync::Arc;

use aero_keys::KeyManager;
use aero_protocol::audit::{ActorType, AuditAction, AuditLogQuery, EventSeverity};
use aero_protocol::keys::Scope;
use aero_protocol::tenancy::Role;
use aero_protocol::AeroError;
use axum::extract::{DefaultBodyLimit, Path, Query, State, WebSocketUpgrade};
use axum::http::{header, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{middleware, Json, Router};
use serde_json::{json, Value};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::app::AeroApp;
use crate::audit::{self, Entry, Who};
use crate::auth::{ClientInfo, Ctx, UserAuth};
use crate::error::{AeroApiError, ApiResult};
use crate::{admin_api, inspector};

#[derive(Clone)]
pub struct AppState {
    pub app: Arc<AeroApp>,
}

/// Largest JSON body for a function deploy (`wasm_bytes` is a JSON number array, ~4× the module).
const DEPLOY_BODY_LIMIT: usize = 48 * 1024 * 1024;
const BODY_LIMIT: usize = 1024 * 1024;

pub fn build_router(app: &Arc<AeroApp>) -> Router {
    let state = AppState { app: app.clone() };

    let v1 = Router::new()
        // Auth
        .route("/auth/register", post(auth_register))
        .route("/auth/login", post(auth_login))
        .route("/auth/refresh", post(auth_refresh))
        .route("/auth/me", get(auth_me))
        .route("/auth/switch-workspace", post(auth_switch_workspace))
        // Workspaces
        .route("/workspaces", post(create_workspace).get(list_workspaces))
        .route(
            "/workspaces/:id",
            get(get_workspace)
                .put(update_workspace)
                .delete(delete_workspace),
        )
        .route("/workspaces/:id/members", get(list_members))
        .route(
            "/workspaces/:id/members/:user_id",
            put(update_member_role).delete(remove_member),
        )
        .route("/workspaces/:id/invite", post(invite_member))
        .route("/workspaces/invite/accept", post(accept_invite))
        // Keys API 2.0
        .route("/keys", post(create_key).get(list_keys))
        .route("/keys/:id", get(get_key).delete(delete_key))
        .route("/keys/verify", post(verify_key))
        // Functions
        .route(
            "/functions",
            post(deploy_function)
                .layer(DefaultBodyLimit::max(DEPLOY_BODY_LIMIT))
                .get(list_functions),
        )
        .route("/functions/:id", get(get_function).delete(delete_function))
        .route("/functions/:id/execute", post(execute_function))
        // Remediation
        .route("/remediation/scan", post(scan_dependencies))
        .route("/remediation/remediate", post(remediate))
        // Skills
        .route("/skills", get(list_skills).post(install_skill))
        .route("/skills/:id", get(get_skill))
        // MCP
        .route("/mcp", get(mcp_ws_handler))
        // AI
        .route("/ai/generate", post(ai_generate))
        // Chat
        .route(
            "/chat/conversations",
            post(create_conversation).get(list_conversations),
        )
        .route(
            "/chat/conversations/:id",
            get(get_conversation).delete(delete_conversation),
        )
        .route("/chat/conversations/:id/messages", get(get_messages))
        .route("/chat/conversations/:id/send", post(send_message))
        .route(
            "/chat/conversations/:id/panel-config",
            put(update_panel_config),
        )
        .route("/chat/panels", get(list_panels))
        .route("/chat/comfort", get(get_comfort_panel))
        // Inspector
        .route("/inspector/stats", get(inspector_stats))
        .route("/inspector/traces", get(inspector_traces))
        // Audit log
        .route("/audit/events", get(audit_events))
        .route("/audit/recent", get(audit_recent))
        .route("/audit/stream", get(audit_stream))
        // Metrics (platform-wide)
        .route("/metrics/snapshot", get(metrics_snapshot))
        .route("/metrics/sandboxes", get(metrics_sandboxes))
        .route("/metrics/sandboxes/:id", get(metrics_sandbox_detail))
        .route("/metrics/history", get(metrics_history));

    let mut router = Router::new()
        .route("/health", get(health))
        .nest("/v1", v1)
        .nest("/api", admin_api::routes())
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            inspector::record,
        ))
        .layer(TraceLayer::new_for_http());

    if let Some(cors) = cors_layer(&app.config.server.cors_origins) {
        router = router.layer(cors);
    }
    router.with_state(state)
}

/// CORS only for the origins in `AERO_CORS_ORIGINS`; none configured = same-origin only.
fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    let list: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o.trim_end_matches('/')).ok())
        .collect();
    if list.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(list))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                HeaderName::from_static("x-aero-key"),
                HeaderName::from_static("x-aero-workspace"),
            ]),
    )
}

async fn health() -> &'static str {
    "AeroBaaS OK"
}

fn forbidden(msg: &str) -> AeroApiError {
    AeroError::Forbidden(msg.into()).into()
}

// === Auth ===

async fn auth_register(
    State(state): State<AppState>,
    client: ClientInfo,
    Json(req): Json<aero_protocol::auth::RegisterRequest>,
) -> ApiResult<Json<aero_protocol::auth::AuthResponse>> {
    let resp = state.app.auth.register(req).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            resp.default_workspace.as_ref().map(|w| w.id),
            Who::user(resp.user.id, &resp.user.email),
            AuditAction::Register,
            "user",
        )
        .id(resp.user.id),
    )
    .await;
    Ok(Json(resp))
}

async fn auth_login(
    State(state): State<AppState>,
    client: ClientInfo,
    Json(req): Json<aero_protocol::auth::LoginRequest>,
) -> ApiResult<Json<aero_protocol::auth::AuthResponse>> {
    let email = req.email.trim().to_lowercase();
    match state.app.auth.login(req, client.ip.as_deref()).await {
        Ok(resp) => {
            audit::record(
                &state.app,
                &client,
                Entry::new(
                    resp.default_workspace.as_ref().map(|w| w.id),
                    Who::user(resp.user.id, &resp.user.email),
                    AuditAction::Login,
                    "user",
                )
                .id(resp.user.id),
            )
            .await;
            Ok(Json(resp))
        }
        Err(e) => {
            audit::record(
                &state.app,
                &client,
                Entry::new(None, Who::anonymous(&email), AuditAction::Login, "user")
                    .details(json!({ "result": "failed", "reason": e.code() }))
                    .severity(EventSeverity::Security),
            )
            .await;
            Err(e.into())
        }
    }
}

async fn auth_refresh(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::auth::RefreshTokenRequest>,
) -> ApiResult<Json<aero_protocol::auth::AuthResponse>> {
    Ok(Json(state.app.auth.refresh(req).await?))
}

async fn auth_me(
    State(state): State<AppState>,
    user: UserAuth,
) -> ApiResult<Json<aero_protocol::auth::UserPublic>> {
    Ok(Json(state.app.auth.get_user(user.user_id).await?))
}

async fn auth_switch_workspace(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Json(req): Json<aero_protocol::auth::SwitchWorkspaceRequest>,
) -> ApiResult<Json<aero_protocol::auth::SwitchWorkspaceResponse>> {
    let ws = req.workspace_id;
    let resp = state.app.auth.switch_workspace(user.user_id, req).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ws),
            Who::user(user.user_id, &user.email),
            AuditAction::WorkspaceSwitched,
            "workspace",
        )
        .id(ws),
    )
    .await;
    Ok(Json(resp))
}

// === Workspaces ===

async fn create_workspace(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Json(req): Json<aero_protocol::tenancy::CreateWorkspaceRequest>,
) -> ApiResult<Json<aero_protocol::tenancy::Workspace>> {
    let ws = state
        .app
        .tenancy
        .create_workspace(user.user_id, req)
        .await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ws.id),
            Who::user(user.user_id, &user.email),
            AuditAction::WorkspaceCreated,
            "workspace",
        )
        .id(ws.id)
        .details(json!({ "name": ws.name, "slug": ws.slug })),
    )
    .await;
    Ok(Json(ws))
}

async fn list_workspaces(
    State(state): State<AppState>,
    user: UserAuth,
) -> ApiResult<Json<Vec<aero_protocol::tenancy::Workspace>>> {
    let list = state.app.tenancy.list_workspaces(user.user_id).await?;
    Ok(Json(list.into_iter().map(|(w, _)| w).collect()))
}

async fn get_workspace(
    State(state): State<AppState>,
    user: UserAuth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<aero_protocol::tenancy::Workspace>> {
    state.app.tenancy.access(id, user.user_id).await?;
    Ok(Json(state.app.tenancy.get_workspace(id).await?))
}

async fn update_workspace(
    State(state): State<AppState>,
    user: UserAuth,
    Path(id): Path<Uuid>,
    Json(req): Json<aero_protocol::tenancy::UpdateWorkspaceRequest>,
) -> ApiResult<Json<aero_protocol::tenancy::Workspace>> {
    let me = state.app.tenancy.access(id, user.user_id).await?;
    Ok(Json(state.app.tenancy.update_workspace(&me, req).await?))
}

async fn delete_workspace(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let me = state.app.tenancy.access(id, user.user_id).await?;
    state.app.tenancy.delete_workspace(&me).await?;
    // The workspace's own audit log is deleted with it; keep a platform-level record.
    audit::record(
        &state.app,
        &client,
        Entry::new(
            None,
            Who::user(user.user_id, &user.email),
            AuditAction::WorkspaceDeleted,
            "workspace",
        )
        .id(id),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_members(
    State(state): State<AppState>,
    user: UserAuth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    state.app.tenancy.access(id, user.user_id).await?;
    let members = state.app.tenancy.list_members(id).await?;
    let total = members.len();
    Ok(Json(json!({ "members": members, "total": total })))
}

async fn update_member_role(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path((id, target_user)): Path<(Uuid, Uuid)>,
    Json(req): Json<Value>,
) -> ApiResult<Json<aero_protocol::tenancy::Membership>> {
    let me = state.app.tenancy.access(id, user.user_id).await?;
    let role = req
        .get("role")
        .and_then(|v| v.as_str())
        .and_then(Role::parse)
        .ok_or_else(|| {
            AeroApiError::BadRequest("role must be owner, admin, manager or user".into())
        })?;
    let target = state
        .app
        .tenancy
        .membership_for_user(id, target_user)
        .await?;
    let old = target.role.clone();
    let m = state.app.tenancy.change_role(&me, target.id, role).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(id),
            Who::user(user.user_id, &user.email),
            AuditAction::MemberRoleChanged,
            "membership",
        )
        .id(m.id)
        .details(json!({ "user_id": m.user_id, "from": old.as_str(), "to": m.role.as_str() })),
    )
    .await;
    Ok(Json(m))
}

async fn remove_member(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path((id, target_user)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    let me = state.app.tenancy.access(id, user.user_id).await?;
    let target = state
        .app
        .tenancy
        .membership_for_user(id, target_user)
        .await?;
    let removed = state.app.tenancy.remove_member(&me, target.id).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(id),
            Who::user(user.user_id, &user.email),
            AuditAction::MemberRemoved,
            "membership",
        )
        .id(removed.id)
        .details(json!({ "user_id": removed.user_id, "role": removed.role.as_str() })),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn invite_member(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path(id): Path<Uuid>,
    Json(req): Json<aero_protocol::tenancy::InviteRequest>,
) -> ApiResult<Json<aero_protocol::tenancy::InviteResponse>> {
    let me = state.app.tenancy.access(id, user.user_id).await?;
    let resp = state.app.tenancy.invite_member(&me, req).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(id),
            Who::user(user.user_id, &user.email),
            AuditAction::MemberInvited,
            "invite",
        )
        .id(resp.invite_id)
        .details(json!({ "email": resp.email, "role": resp.role.as_str() })),
    )
    .await;
    Ok(Json(resp))
}

async fn accept_invite(
    State(state): State<AppState>,
    user: UserAuth,
    Json(req): Json<aero_protocol::tenancy::AcceptInviteRequest>,
) -> ApiResult<Json<aero_protocol::tenancy::Membership>> {
    let (m, _) = state
        .app
        .tenancy
        .accept_invite(&req.invite_token, user.user_id, &user.email)
        .await?;
    Ok(Json(m))
}

// === Keys ===

async fn create_key(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    Json(req): Json<aero_protocol::keys::CreateKeyRequest>,
) -> ApiResult<Json<aero_protocol::keys::CreateKeyResponse>> {
    let me = ctx.require_role(Role::Manager)?;
    if req.scopes.contains(&Scope::Admin) && !me.role.implies(&Role::Admin) {
        return Err(forbidden(
            "Only admins and owners can create keys with the admin scope",
        ));
    }
    let resp = state
        .app
        .key_manager
        .create_key(ctx.ws, Some(me.user_id), req)
        .await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ctx.ws),
            Who::from(&ctx),
            AuditAction::KeyCreated,
            "api_key",
        )
        .id(resp.id)
        .details(json!({
            "prefix": resp.key_prefix,
            "scopes": resp.scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "credits_limit": resp.credits_limit,
        })),
    )
    .await;
    Ok(Json(resp))
}

async fn list_keys(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<Vec<aero_protocol::keys::ApiKey>>> {
    ctx.require_role(Role::Manager)?;
    Ok(Json(state.app.key_manager.list_keys(ctx.ws).await?))
}

async fn get_key(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<aero_protocol::keys::ApiKey>> {
    ctx.require_role(Role::Manager)?;
    Ok(Json(state.app.key_manager.get_key(ctx.ws, id).await?))
}

async fn delete_key(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    ctx.require_role(Role::Manager)?;
    state.app.key_manager.delete_key(ctx.ws, id).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ctx.ws),
            Who::from(&ctx),
            AuditAction::KeyDeleted,
            "api_key",
        )
        .id(id),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn verify_key(
    State(state): State<AppState>,
    Json(req): Json<aero_protocol::keys::VerifyKeyRequest>,
) -> ApiResult<Json<aero_protocol::keys::VerifyKeyResponse>> {
    Ok(Json(state.app.key_manager.verify_key(req).await?))
}

// === Functions ===

/// Keys with a function allow-list only see the functions on it (404 for the rest).
fn function_visible(ctx: &Ctx, id: Uuid) -> ApiResult<()> {
    match ctx.key() {
        Some(k) if !KeyManager::function_allowed(k, id) => {
            Err(AeroError::NotFound(format!("Function {id} not found")).into())
        }
        _ => Ok(()),
    }
}

async fn deploy_function(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    Json(req): Json<aero_protocol::functions::DeployRequest>,
) -> ApiResult<Json<aero_protocol::functions::DeployResponse>> {
    ctx.require(Role::User, Scope::FunctionsWrite)?;
    let (function, resp) = state
        .app
        .wasm_runtime
        .deploy(ctx.ws, &req, &state.app.remediation)
        .await?;
    state
        .app
        .db
        .store_function(&function, &req.wasm_bytes, ctx.user_id())
        .await?;
    let (action, severity) = if resp.quarantine_passed {
        (AuditAction::FunctionDeployed, EventSeverity::Info)
    } else {
        (AuditAction::FunctionBlocked, EventSeverity::Security)
    };
    audit::record(
        &state.app,
        &client,
        Entry::new(Some(ctx.ws), Who::from(&ctx), action, "function")
            .id(function.id)
            .details(json!({ "name": function.name, "wasm_hash": function.wasm_hash }))
            .severity(severity),
    )
    .await;
    if !resp.quarantine_passed {
        audit::record(
            &state.app,
            &client,
            Entry::new(
                Some(ctx.ws),
                Who::module(ActorType::HazmatModule, "hazmat"),
                AuditAction::HazmatViolation,
                "function",
            )
            .id(function.id)
            .severity(EventSeverity::Security),
        )
        .await;
    }
    Ok(Json(resp))
}

async fn list_functions(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<Vec<aero_protocol::functions::Function>>> {
    if ctx.require(Role::User, Scope::FunctionsList).is_err() {
        ctx.require(Role::User, Scope::FunctionsRead)?;
    }
    let mut funcs = state.app.db.list_functions(ctx.ws).await?;
    if let Some(k) = ctx.key() {
        funcs.retain(|f| KeyManager::function_allowed(k, f.id));
    }
    Ok(Json(funcs))
}

async fn get_function(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<aero_protocol::functions::Function>> {
    ctx.require(Role::User, Scope::FunctionsRead)?;
    function_visible(&ctx, id)?;
    Ok(Json(state.app.db.get_function(ctx.ws, id).await?))
}

async fn delete_function(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    ctx.require(Role::Manager, Scope::FunctionsWrite)?;
    function_visible(&ctx, id)?;
    if !state.app.db.delete_function(ctx.ws, id).await? {
        return Err(AeroError::NotFound(format!("Function {id} not found")).into());
    }
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ctx.ws),
            Who::from(&ctx),
            AuditAction::FunctionDeleted,
            "function",
        )
        .id(id),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn execute_function(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    Path(id): Path<Uuid>,
    Json(req): Json<aero_protocol::functions::ExecuteRequest>,
) -> ApiResult<Json<aero_protocol::functions::ExecuteResponse>> {
    ctx.require(Role::User, Scope::ExecutionsWrite)?;
    function_visible(&ctx, id)?;
    let function = state.app.db.get_function(ctx.ws, id).await?;
    let wasm = state.app.db.get_function_wasm(ctx.ws, id).await?;
    let resp = state
        .app
        .wasm_runtime
        .execute(&function, &wasm, req.payload)
        .await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ctx.ws),
            Who::from(&ctx),
            AuditAction::FunctionExecuted,
            "function",
        )
        .id(id)
        .details(json!({ "execution_id": resp.execution_id, "duration_ms": resp.duration_ms })),
    )
    .await;
    Ok(Json(resp))
}

// === Remediation ===

async fn scan_dependencies(
    State(state): State<AppState>,
    ctx: Ctx,
    client: ClientInfo,
    Json(req): Json<aero_protocol::remediation::ScanRequest>,
) -> ApiResult<Json<aero_protocol::remediation::ScanResult>> {
    ctx.require(Role::User, Scope::RemediationRun)?;
    if req.manifest_content.len() > 512 * 1024 {
        return Err(AeroApiError::BadRequest(
            "Manifest is larger than 512 KB".into(),
        ));
    }
    let result = state.app.remediation.scan(req).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ctx.ws),
            Who::from(&ctx),
            AuditAction::ScanCompleted,
            "manifest",
        )
        .details(json!({
            "total": result.total_count,
            "critical": result.critical_count,
        })),
    )
    .await;
    Ok(Json(result))
}

async fn remediate(
    State(state): State<AppState>,
    ctx: Ctx,
    Json(req): Json<aero_protocol::remediation::ScanRequest>,
) -> ApiResult<Json<aero_protocol::remediation::RemediationResult>> {
    ctx.require(Role::User, Scope::RemediationRun)?;
    if req.manifest_content.len() > 512 * 1024 {
        return Err(AeroApiError::BadRequest(
            "Manifest is larger than 512 KB".into(),
        ));
    }
    Ok(Json(state.app.remediation.remediate(req).await?))
}

// === Skills ===

async fn list_skills(
    State(state): State<AppState>,
    _ctx: Ctx,
) -> ApiResult<Json<Vec<aero_protocol::skills::SkillPack>>> {
    Ok(Json(state.app.skill_registry.list_skills().await?))
}

async fn get_skill(
    State(state): State<AppState>,
    _ctx: Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<aero_protocol::skills::SkillPack>> {
    Ok(Json(state.app.skill_registry.get_skill(&id).await?))
}

/// Skill packs are shared by every workspace, so only platform admins may install them.
async fn install_skill(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Json(skill): Json<aero_protocol::skills::SkillPack>,
) -> ApiResult<Json<aero_protocol::skills::SkillPack>> {
    if !state
        .app
        .is_platform_admin(user.user_id, &user.email)
        .await?
    {
        return Err(forbidden("Only platform admins can install skill packs"));
    }
    let skill = state.app.skill_registry.install_skill(skill).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            None,
            Who::user(user.user_id, &user.email),
            AuditAction::SkillInstalled,
            "skill",
        )
        .id(&skill.id),
    )
    .await;
    Ok(Json(skill))
}

// === MCP ===

async fn mcp_ws_handler(
    State(state): State<AppState>,
    ctx: Ctx,
    ws: WebSocketUpgrade,
) -> ApiResult<impl IntoResponse> {
    ctx.require(Role::User, Scope::McpConnect)?;
    Ok(ws.on_upgrade(
        move |socket| async move { state.app.mcp_server.handle_connection(socket).await },
    ))
}

// === AI ===

async fn ai_generate(
    State(state): State<AppState>,
    ctx: Ctx,
    Json(req): Json<Value>,
) -> ApiResult<Json<Value>> {
    ctx.require(Role::User, Scope::AiInference)?;
    let prompt = req
        .get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AeroApiError::BadRequest("Missing 'prompt' field".into()))?;
    if prompt.len() > 32 * 1024 {
        return Err(AeroApiError::BadRequest(
            "Prompt is longer than 32 KB".into(),
        ));
    }
    let result = state.app.ai_engine.generate_text(prompt).await?;
    Ok(Json(json!({ "text": result })))
}

// === Chat (per person, inside the active workspace) ===

async fn own_conversation(
    state: &AppState,
    ctx: &Ctx,
    id: Uuid,
) -> ApiResult<aero_protocol::chat::Conversation> {
    let me = ctx.membership()?;
    let conv = state
        .app
        .chat
        .get_conversation(id)
        .await
        .map_err(|_| AeroError::NotFound("Conversation not found".into()))?;
    if conv.user_id != me.user_id || conv.workspace_id != ctx.ws {
        return Err(AeroError::NotFound("Conversation not found".into()).into());
    }
    Ok(conv)
}

async fn create_conversation(
    State(state): State<AppState>,
    ctx: Ctx,
    Json(req): Json<aero_protocol::chat::CreateConversationRequest>,
) -> ApiResult<Json<aero_protocol::chat::Conversation>> {
    let me = ctx.membership()?;
    let conv = state
        .app
        .chat
        .create_conversation(ctx.ws, me.user_id, req)
        .await?;
    Ok(Json(conv))
}

async fn list_conversations(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<Vec<aero_protocol::chat::ConversationSummary>>> {
    let me = ctx.membership()?;
    let mut convs = state.app.chat.list_conversations(me.user_id).await?;
    let mut mine = Vec::with_capacity(convs.len());
    for c in convs.drain(..) {
        if let Ok(full) = state.app.chat.get_conversation(c.id).await {
            if full.workspace_id == ctx.ws {
                mine.push(c);
            }
        }
    }
    Ok(Json(mine))
}

async fn get_conversation(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<aero_protocol::chat::Conversation>> {
    Ok(Json(own_conversation(&state, &ctx, id).await?))
}

async fn delete_conversation(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    own_conversation(&state, &ctx, id).await?;
    state.app.chat.delete_conversation(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_messages(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<aero_protocol::chat::Message>>> {
    own_conversation(&state, &ctx, id).await?;
    Ok(Json(state.app.chat.get_messages(id).await?))
}

async fn send_message(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
    Json(mut req): Json<aero_protocol::chat::SendMessageRequest>,
) -> ApiResult<Json<aero_protocol::chat::SendMessageResponse>> {
    let conv = own_conversation(&state, &ctx, id).await?;
    req.conversation_id = id;
    Ok(Json(state.app.chat.send_message(conv.user_id, req).await?))
}

async fn update_panel_config(
    State(state): State<AppState>,
    ctx: Ctx,
    Path(id): Path<Uuid>,
    Json(req): Json<aero_protocol::chat::PanelConfig>,
) -> ApiResult<Json<aero_protocol::chat::Conversation>> {
    own_conversation(&state, &ctx, id).await?;
    Ok(Json(state.app.chat.update_panel_config(id, req).await?))
}

async fn list_panels(_user: UserAuth) -> ApiResult<Json<Vec<aero_protocol::chat::Panel>>> {
    Ok(Json(aero_chat::PanelManager::default_panels()))
}

async fn get_comfort_panel(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<aero_protocol::chat::ComfortPanel>> {
    let me = ctx.membership()?;
    let user = state.app.auth.get_user(me.user_id).await?;
    let conversations = state.app.chat.list_conversations(me.user_id).await?;
    let stats = aero_protocol::chat::ComfortStats {
        total_conversations: conversations.len() as u32,
        total_tokens: 0,
        total_deployments: 0,
        avg_latency_ms: 0.0,
        uptime_hours: 0.0,
    };
    let panel =
        aero_chat::PanelManager::build_comfort_panel(Some(&user.name), &conversations, stats);
    Ok(Json(panel))
}

// === Inspector ===

async fn inspector_stats(
    State(state): State<AppState>,
    ctx: Ctx,
) -> ApiResult<Json<aero_protocol::inspector::InspectorStats>> {
    ctx.require_role(Role::Manager)?;
    Ok(Json(inspector::v1_stats(&state.app, ctx.ws).await?))
}

async fn inspector_traces(
    State(state): State<AppState>,
    ctx: Ctx,
    Query(q): Query<aero_protocol::inspector::InspectorQuery>,
) -> ApiResult<Json<Vec<aero_protocol::inspector::RequestTrace>>> {
    ctx.require_role(Role::Manager)?;
    Ok(Json(inspector::traces(&state.app, ctx.ws, &q).await?))
}

// === Audit log ===

async fn audit_events(
    State(state): State<AppState>,
    ctx: Ctx,
    Query(q): Query<AuditLogQuery>,
) -> ApiResult<Json<aero_protocol::audit::AuditLogResponse>> {
    ctx.require_role(Role::Manager)?;
    Ok(Json(audit::query(&state.app, ctx.ws, &q).await?))
}

#[derive(serde::Deserialize)]
struct RecentQuery {
    limit: Option<usize>,
}

async fn audit_recent(
    State(state): State<AppState>,
    ctx: Ctx,
    Query(q): Query<RecentQuery>,
) -> ApiResult<Json<Vec<aero_protocol::audit::AuditEvent>>> {
    ctx.require_role(Role::Manager)?;
    let q = AuditLogQuery {
        workspace_id: None,
        actor_type: None,
        action: None,
        severity: None,
        start_time: None,
        end_time: None,
        limit: Some(q.limit.unwrap_or(50)),
        offset: None,
    };
    Ok(Json(audit::query(&state.app, ctx.ws, &q).await?.events))
}

/// Live audit events of the active workspace (browsers: `?token=…&workspace=…`).
async fn audit_stream(
    State(state): State<AppState>,
    ctx: Ctx,
    ws: WebSocketUpgrade,
) -> ApiResult<impl IntoResponse> {
    ctx.require_role(Role::Manager)?;
    let workspace = ctx.ws;
    Ok(ws.on_upgrade(move |mut socket| async move {
        let mut rx = state.app.event_broadcaster.subscribe();
        loop {
            match rx.recv().await {
                Ok(ev) if ev.event.workspace_id == workspace => {
                    let msg = serde_json::to_string(&ev).unwrap_or_default();
                    if socket
                        .send(axum::extract::ws::Message::Text(msg))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => break,
            }
        }
        state.app.event_broadcaster.unsubscribe();
    }))
}

// === Metrics (platform-wide: platform admins only) ===

async fn platform_admin(state: &AppState, user: &UserAuth) -> ApiResult<()> {
    if state
        .app
        .is_platform_admin(user.user_id, &user.email)
        .await?
    {
        Ok(())
    } else {
        Err(forbidden("Platform metrics are for platform admins"))
    }
}

async fn metrics_snapshot(
    State(state): State<AppState>,
    user: UserAuth,
) -> ApiResult<Json<aero_protocol::metrics::MetricsSnapshot>> {
    platform_admin(&state, &user).await?;
    Ok(Json(state.app.metrics_collector.snapshot()))
}

async fn metrics_sandboxes(
    State(state): State<AppState>,
    user: UserAuth,
) -> ApiResult<Json<Vec<aero_protocol::metrics::SandboxMetrics>>> {
    platform_admin(&state, &user).await?;
    Ok(Json(state.app.metrics_collector.list_sandboxes()))
}

async fn metrics_sandbox_detail(
    State(state): State<AppState>,
    user: UserAuth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<aero_protocol::metrics::SandboxMetrics>> {
    platform_admin(&state, &user).await?;
    let m = state
        .app
        .metrics_collector
        .get_sandbox(id)
        .ok_or_else(|| AeroError::NotFound("Sandbox not found".into()))?;
    Ok(Json(m))
}

async fn metrics_history(
    State(state): State<AppState>,
    user: UserAuth,
    Query(q): Query<aero_protocol::metrics::MetricsQuery>,
) -> ApiResult<Json<aero_protocol::metrics::MetricsHistory>> {
    platform_admin(&state, &user).await?;
    let window = q.window_minutes.unwrap_or(60);
    Ok(Json(state.app.metrics_history.get_history(window)))
}
