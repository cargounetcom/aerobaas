//! `/api/*` — the API contract of the SovereignBaaS Admin (`cargounetcom/sovereign-admin`,
//! `inspector/src`), so the same admin UI can run on AeroBaaS. Shapes and role rules match it;
//! workspace ids are UUIDs and sign-in uses AeroBaaS tokens.
//!
//! AI provider routes (`/api/workspaces/{ws}/ai/*`) arrive with the real AI layer in Phase 2.

use aero_protocol::audit::AuditAction;
use aero_protocol::tenancy::{CreateWorkspaceRequest, InviteRequest, Membership, Role};
use aero_protocol::AeroError;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::audit::{self, Entry, Who};
use crate::auth::{ClientInfo, UserAuth};
use crate::error::{AeroApiError, ApiResult};
use crate::inspector;
use crate::router::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/platform/me", get(platform_me))
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route("/invites/:token", get(invite_info))
        .route("/invites/:token/accept", post(accept_invite))
        .route("/workspaces/:ws/me", get(my_access))
        .route("/workspaces/:ws/members", get(members))
        .route(
            "/workspaces/:ws/members/:membership",
            patch(change_role).delete(remove_member),
        )
        .route(
            "/workspaces/:ws/invites",
            get(list_invites).post(create_invite),
        )
        .route("/workspaces/:ws/invites/:id", delete(revoke_invite))
        .route("/workspaces/:ws/inspector/requests", get(inspector_list))
        .route("/workspaces/:ws/inspector/stats", get(inspector_stats))
}

async fn member(state: &AppState, user: &UserAuth, ws: Uuid) -> ApiResult<Membership> {
    Ok(state.app.tenancy.access(ws, user.user_id).await?)
}

/// GET /api/me
async fn me(State(state): State<AppState>, user: UserAuth) -> ApiResult<Json<Value>> {
    let u = state.app.auth.get_user(user.user_id).await?;
    Ok(Json(
        json!({ "id": u.id, "name": u.name, "email": u.email }),
    ))
}

/// GET /api/platform/me
async fn platform_me(State(state): State<AppState>, user: UserAuth) -> ApiResult<Json<Value>> {
    let admin = state
        .app
        .is_platform_admin(user.user_id, &user.email)
        .await?;
    Ok(Json(json!({ "platformAdmin": admin, "edition": "aero" })))
}

/// GET /api/workspaces — the caller's workspaces (the Appwrite edition reads teams instead).
async fn list_workspaces(State(state): State<AppState>, user: UserAuth) -> ApiResult<Json<Value>> {
    let list = state.app.tenancy.list_workspaces(user.user_id).await?;
    let items: Vec<Value> = list
        .into_iter()
        .map(|(w, role)| {
            json!({
                "id": w.id, "name": w.name, "slug": w.slug, "plan": w.plan,
                "members": w.member_count, "role": role,
            })
        })
        .collect();
    Ok(Json(json!({ "workspaces": items })))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
    slug: Option<String>,
}

/// POST /api/workspaces — `{name, slug?}`; a slug is generated from the name when missing.
async fn create_workspace(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Json(body): Json<CreateBody>,
) -> ApiResult<Json<Value>> {
    let slug = body.slug.unwrap_or_else(|| {
        let base: String = body
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base: String = base.chars().take(32).collect();
        let suffix = &Uuid::new_v4().simple().to_string()[..6];
        if base.is_empty() {
            format!("ws-{suffix}")
        } else {
            format!("{base}-{suffix}")
        }
    });
    let w = state
        .app
        .tenancy
        .create_workspace(
            user.user_id,
            CreateWorkspaceRequest {
                name: body.name,
                slug,
                plan: None,
            },
        )
        .await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(w.id),
            Who::user(user.user_id, &user.email),
            AuditAction::WorkspaceCreated,
            "workspace",
        )
        .id(w.id),
    )
    .await;
    Ok(Json(
        json!({ "id": w.id, "name": w.name, "slug": w.slug, "plan": w.plan, "members": 1, "role": Role::Owner }),
    ))
}

/// GET /api/workspaces/{ws}/me — caller's role and permissions.
async fn my_access(
    State(state): State<AppState>,
    user: UserAuth,
    Path(ws): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let m = member(&state, &user, ws).await?;
    let r = &m.role;
    let invite_roles: Vec<Role> = [Role::Owner, Role::Admin, Role::Manager, Role::User]
        .into_iter()
        .filter(|t| r.can_invite_role(t))
        .collect();
    Ok(Json(json!({
        "workspaceId": ws,
        "userId": user.user_id,
        "membershipId": m.id,
        "role": r,
        "permissions": {
            "inviteRoles": invite_roles,
            "viewInspector": r.implies(&Role::Manager),
            "manageAi": r.implies(&Role::Admin),
            "viewInvites": r.implies(&Role::Manager),
            "manageMembers": r.implies(&Role::Admin),
            "deleteWorkspace": *r == Role::Owner,
        }
    })))
}

/// GET /api/workspaces/{ws}/members
async fn members(
    State(state): State<AppState>,
    user: UserAuth,
    Path(ws): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    member(&state, &user, ws).await?;
    let list = state.app.tenancy.list_members(ws).await?;
    let out: Vec<Value> = list
        .into_iter()
        .map(|m| {
            json!({
                "membershipId": m.membership.id,
                "userId": m.membership.user_id,
                "name": m.name,
                "email": m.email,
                "role": m.membership.role,
                "joined": m.membership.joined_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
                "confirmed": true,
            })
        })
        .collect();
    Ok(Json(json!({ "members": out })))
}

#[derive(Deserialize)]
struct RoleBody {
    role: String,
}

/// PATCH /api/workspaces/{ws}/members/{membership}
async fn change_role(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path((ws, membership_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<RoleBody>,
) -> ApiResult<Json<Value>> {
    let me = member(&state, &user, ws).await?;
    let role =
        Role::parse(&body.role).ok_or_else(|| AeroApiError::BadRequest("Unknown role".into()))?;
    let m = state
        .app
        .tenancy
        .change_role(&me, membership_id, role)
        .await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ws),
            Who::user(user.user_id, &user.email),
            AuditAction::MemberRoleChanged,
            "membership",
        )
        .id(m.id)
        .details(json!({ "user_id": m.user_id, "to": m.role.as_str() })),
    )
    .await;
    Ok(Json(json!({ "membershipId": m.id, "role": m.role })))
}

/// DELETE /api/workspaces/{ws}/members/{membership}
async fn remove_member(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path((ws, membership_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Value>> {
    let me = member(&state, &user, ws).await?;
    let removed = state.app.tenancy.remove_member(&me, membership_id).await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ws),
            Who::user(user.user_id, &user.email),
            AuditAction::MemberRemoved,
            "membership",
        )
        .id(removed.id)
        .details(json!({ "user_id": removed.user_id })),
    )
    .await;
    Ok(Json(json!({ "removed": membership_id })))
}

#[derive(Deserialize)]
struct InviteBody {
    email: String,
    role: String,
}

/// POST /api/workspaces/{ws}/invites → `{id, email, role, expiresAt, link}`
async fn create_invite(
    State(state): State<AppState>,
    user: UserAuth,
    client: ClientInfo,
    Path(ws): Path<Uuid>,
    Json(body): Json<InviteBody>,
) -> ApiResult<Json<Value>> {
    let me = member(&state, &user, ws).await?;
    let role =
        Role::parse(&body.role).ok_or_else(|| AeroApiError::BadRequest("Unknown role".into()))?;
    let inv = state
        .app
        .tenancy
        .invite_member(
            &me,
            InviteRequest {
                email: body.email,
                role,
            },
        )
        .await?;
    audit::record(
        &state.app,
        &client,
        Entry::new(
            Some(ws),
            Who::user(user.user_id, &user.email),
            AuditAction::MemberInvited,
            "invite",
        )
        .id(inv.invite_id)
        .details(json!({ "email": inv.email, "role": inv.role.as_str() })),
    )
    .await;
    Ok(Json(json!({
        "id": inv.invite_id,
        "email": inv.email,
        "role": inv.role,
        "expiresAt": inv.expires_at.to_rfc3339(),
        "link": format!("{}/invite/{}", state.app.config.server.public_url, inv.invite_token),
    })))
}

/// GET /api/workspaces/{ws}/invites — open invites.
async fn list_invites(
    State(state): State<AppState>,
    user: UserAuth,
    Path(ws): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let me = member(&state, &user, ws).await?;
    let list = state.app.tenancy.list_invites(&me).await?;
    let out: Vec<Value> = list
        .into_iter()
        .map(|i| {
            json!({
                "id": i.id, "email": i.email, "role": i.role, "invitedBy": i.invited_by,
                "createdAt": i.created_at.to_rfc3339(), "expiresAt": i.expires_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(json!({ "invites": out })))
}

/// DELETE /api/workspaces/{ws}/invites/{id}
async fn revoke_invite(
    State(state): State<AppState>,
    user: UserAuth,
    Path((ws, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<Value>> {
    let me = member(&state, &user, ws).await?;
    state.app.tenancy.revoke_invite(&me, id).await?;
    Ok(Json(json!({ "revoked": id })))
}

/// GET /api/invites/{token} — public: what this invite is for.
async fn invite_info(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    let i = state.app.tenancy.invite_info(&token).await?;
    Ok(Json(json!({
        "workspaceId": i.workspace_id,
        "workspaceName": i.workspace_name,
        "email": i.email,
        "role": i.role,
        "expiresAt": i.expires_at.to_rfc3339(),
        "status": i.status,
    })))
}

/// POST /api/invites/{token}/accept
async fn accept_invite(
    State(state): State<AppState>,
    user: UserAuth,
    Path(token): Path<String>,
) -> ApiResult<Json<Value>> {
    let (m, i) = state
        .app
        .tenancy
        .accept_invite(&token, user.user_id, &user.email)
        .await?;
    Ok(Json(json!({
        "workspaceId": i.workspace_id,
        "workspaceName": i.workspace_name,
        "membershipId": m.id,
        "role": m.role,
    })))
}

/// GET /api/workspaces/{ws}/inspector/requests (Manager role or higher)
async fn inspector_list(
    State(state): State<AppState>,
    user: UserAuth,
    Path(ws): Path<Uuid>,
    Query(q): Query<inspector::ListQuery>,
) -> ApiResult<Json<Value>> {
    let m = member(&state, &user, ws).await?;
    if !m.role.implies(&Role::Manager) {
        return Err(
            AeroError::Forbidden("Inspector requires Manager role or higher".into()).into(),
        );
    }
    Ok(Json(inspector::list(&state.app, ws, &q).await?))
}

/// GET /api/workspaces/{ws}/inspector/stats (every member)
async fn inspector_stats(
    State(state): State<AppState>,
    user: UserAuth,
    Path(ws): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    member(&state, &user, ws).await?;
    Ok(Json(inspector::stats(&state.app, ws).await?))
}
