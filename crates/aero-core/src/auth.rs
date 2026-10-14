//! Request authentication.
//!
//! * [`UserAuth`] — a signed-in person (JWT access token).
//! * [`Ctx`] — a workspace context: a member (JWT + membership check) **or** an API key
//!   (`Authorization: Bearer aero_…` or `X-Aero-Key`). Every key request costs one credit.
//!
//! The active workspace for a person is `X-Aero-Workspace` when sent, else the token's
//! `workspace_id` (set at login / switch-workspace). Membership is re-checked on every request.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use aero_keys::KeyManager;
use aero_protocol::audit::ActorType;
use aero_protocol::keys::{ApiKey, Scope};
use aero_protocol::tenancy::{Membership, Role};
use aero_protocol::AeroError;
use axum::async_trait;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::{header, request::Parts, HeaderMap};
use uuid::Uuid;

use crate::error::AeroApiError;
use crate::router::AppState;

/// Filled in by extractors while a request runs; read by the request inspector.
#[derive(Clone, Default, Debug)]
pub struct MetaHandle(Arc<Mutex<Meta>>);

#[derive(Default, Debug, Clone)]
pub struct Meta {
    pub user_id: Option<Uuid>,
    pub key_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub detail: Option<String>,
}

impl MetaHandle {
    fn with(&self, f: impl FnOnce(&mut Meta)) {
        if let Ok(mut m) = self.0.lock() {
            f(&mut m)
        }
    }
    pub fn set_user(&self, id: Uuid) {
        self.with(|m| m.user_id = Some(id));
    }
    pub fn set_key(&self, id: Uuid) {
        self.with(|m| m.key_id = Some(id));
    }
    pub fn set_workspace(&self, id: Uuid) {
        self.with(|m| m.workspace_id = Some(id));
    }
    pub fn set_detail(&self, d: impl Into<String>) {
        let d = d.into();
        self.with(|m| m.detail = Some(d));
    }
    pub fn snapshot(&self) -> Meta {
        self.0.lock().map(|m| m.clone()).unwrap_or_default()
    }
}

fn meta(parts: &Parts) -> Option<MetaHandle> {
    parts.extensions.get::<MetaHandle>().cloned()
}

/// Client IP: the socket peer, or `X-Forwarded-For`/`X-Real-IP` when `AERO_TRUST_PROXY=true`.
pub fn client_ip(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trust_proxy: bool,
) -> Option<String> {
    if trust_proxy {
        let fwd = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                headers
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string)
            });
        if fwd.is_some() {
            return fwd;
        }
    }
    peer.map(|p| p.ip().to_string())
}

/// Caller IP and user agent (for audit records and login limits).
#[derive(Clone, Debug, Default)]
pub struct ClientInfo {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

#[async_trait]
impl FromRequestParts<AppState> for ClientInfo {
    type Rejection = AeroApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0);
        Ok(ClientInfo {
            ip: client_ip(&parts.headers, peer, state.app.config.server.trust_proxy),
            user_agent: parts
                .headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.chars().take(300).collect()),
        })
    }
}

/// The credential sent with the request. Query-string tokens (`?token=`) are accepted only on
/// WebSocket upgrades, where browsers cannot set headers.
fn credential(parts: &Parts) -> Option<String> {
    if let Some(k) = parts
        .headers
        .get("x-aero-key")
        .and_then(|v| v.to_str().ok())
    {
        return Some(k.trim().to_string()).filter(|s| !s.is_empty());
    }
    if let Some(v) = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
    {
        return Some(v.trim().to_string()).filter(|s| !s.is_empty());
    }
    let is_ws = parts
        .headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    if is_ws {
        return query_param(parts, "token");
    }
    None
}

pub fn query_param(parts: &Parts, name: &str) -> Option<String> {
    parts.uri.query().and_then(|q| {
        q.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == name).then(|| v.to_string()).filter(|v| !v.is_empty())
        })
    })
}

fn unauthorized(msg: &str) -> AeroApiError {
    AeroError::Unauthorized(msg.into()).into()
}

/// A signed-in person (API keys are rejected).
#[derive(Clone, Debug)]
pub struct UserAuth {
    pub user_id: Uuid,
    pub email: String,
    /// Workspace selected in the token (may be stale; check membership before use).
    pub token_workspace: Option<Uuid>,
}

#[async_trait]
impl FromRequestParts<AppState> for UserAuth {
    type Rejection = AeroApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = credential(parts).ok_or_else(|| unauthorized("Sign in first"))?;
        if token.starts_with("aero_") {
            return Err(AeroError::Forbidden(
                "This endpoint needs a signed-in user, not an API key".into(),
            )
            .into());
        }
        let claims = state.app.auth.verify_token(&token)?;
        if let Some(m) = meta(parts) {
            m.set_user(claims.sub);
        }
        Ok(UserAuth {
            user_id: claims.sub,
            email: claims.email,
            token_workspace: claims.workspace_id,
        })
    }
}

/// Who is acting inside a workspace.
#[derive(Clone, Debug)]
pub enum Actor {
    Member {
        email: String,
        membership: Membership,
    },
    Key(Box<ApiKey>),
}

/// A request inside one workspace, already authorised as a member or an API key of it.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub ws: Uuid,
    pub actor: Actor,
}

impl Ctx {
    pub fn user_id(&self) -> Option<Uuid> {
        match &self.actor {
            Actor::Member { membership, .. } => Some(membership.user_id),
            Actor::Key(_) => None,
        }
    }

    pub fn membership(&self) -> Result<&Membership, AeroError> {
        match &self.actor {
            Actor::Member { membership, .. } => Ok(membership),
            Actor::Key(_) => Err(AeroError::Forbidden(
                "This endpoint needs a signed-in user, not an API key".into(),
            )),
        }
    }

    pub fn key(&self) -> Option<&ApiKey> {
        match &self.actor {
            Actor::Key(k) => Some(k),
            Actor::Member { .. } => None,
        }
    }

    /// People need at least `min`; keys are refused.
    pub fn require_role(&self, min: Role) -> Result<&Membership, AeroError> {
        let m = self.membership()?;
        if m.role.implies(&min) {
            Ok(m)
        } else {
            Err(AeroError::Forbidden(format!(
                "Requires {} role or higher",
                capitalise(min.as_str())
            )))
        }
    }

    /// People need at least `min`; keys need `scope`.
    pub fn require(&self, min: Role, scope: Scope) -> Result<(), AeroError> {
        match &self.actor {
            Actor::Member { .. } => self.require_role(min).map(|_| ()),
            Actor::Key(k) => KeyManager::check_scope(k, &scope),
        }
    }

    pub fn actor_type(&self) -> ActorType {
        match self.actor {
            Actor::Member { .. } => ActorType::User,
            Actor::Key(_) => ActorType::AiAgent,
        }
    }

    pub fn actor_id(&self) -> Option<Uuid> {
        match &self.actor {
            Actor::Member { membership, .. } => Some(membership.user_id),
            Actor::Key(k) => Some(k.id),
        }
    }

    pub fn actor_name(&self) -> String {
        match &self.actor {
            Actor::Member { email, .. } => email.clone(),
            Actor::Key(k) => format!("key:{} ({})", k.name, k.key_prefix),
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

#[async_trait]
impl FromRequestParts<AppState> for Ctx {
    type Rejection = AeroApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = credential(parts).ok_or_else(|| unauthorized("Sign in or send an API key"))?;
        let m = meta(parts);
        if token.starts_with("aero_") {
            let key = state.app.key_manager.authenticate(&token, 1).await?;
            if let Some(m) = &m {
                m.set_key(key.id);
                m.set_workspace(key.project_id);
            }
            return Ok(Ctx {
                ws: key.project_id,
                actor: Actor::Key(Box::new(key)),
            });
        }
        let claims = state.app.auth.verify_token(&token)?;
        if let Some(m) = &m {
            m.set_user(claims.sub);
        }
        let header_ws = parts
            .headers
            .get("x-aero-workspace")
            .and_then(|v| v.to_str().ok());
        let ws = match header_ws {
            Some(h) => Uuid::parse_str(h.trim())
                .map_err(|_| AeroApiError::BadRequest("X-Aero-Workspace must be a workspace id".into()))?,
            None => match query_param(parts, "workspace").and_then(|q| Uuid::parse_str(&q).ok()) {
                Some(q) => q,
                None => claims.workspace_id.ok_or_else(|| {
                    AeroApiError::BadRequest(
                        "No active workspace: send X-Aero-Workspace or call /v1/auth/switch-workspace".into(),
                    )
                })?,
            },
        };
        let membership = state.app.tenancy.access(ws, claims.sub).await?;
        if let Some(m) = &m {
            m.set_workspace(ws);
        }
        Ok(Ctx {
            ws,
            actor: Actor::Member {
                email: claims.email,
                membership,
            },
        })
    }
}
