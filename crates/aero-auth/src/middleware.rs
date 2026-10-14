use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

use aero_protocol::auth::JwtClaims;

use crate::jwt::JwtService;

pub struct AuthState {
    pub jwt: Arc<JwtService>,
}

pub async fn auth_middleware(
    State(auth_state): State<AuthState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = auth_state
        .jwt
        .verify(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub async fn require_role(
    req: Request<axum::body::Body>,
    next: Next,
    min_role: aero_protocol::tenancy::Role,
) -> Result<Response, StatusCode> {
    let claims = req
        .extensions()
        .get::<JwtClaims>()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let user_role = claims
        .role
        .as_deref()
        .map(|r| match r {
            "owner" => aero_protocol::tenancy::Role::Owner,
            "admin" => aero_protocol::tenancy::Role::Admin,
            "manager" => aero_protocol::tenancy::Role::Manager,
            _ => aero_protocol::tenancy::Role::User,
        })
        .unwrap_or(aero_protocol::tenancy::Role::User);

    if user_role.implies(&min_role) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

use axum::extract::State;
