use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

use aero_protocol::keys::Scope;

pub async fn require_scope(
    req: Request<axum::body::Body>,
    next: Next,
    required_scope: Scope,
) -> Result<Response, StatusCode> {
    let scope_header = req
        .headers()
        .get("x-aero-scope")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if scope_header == Scope::Admin.as_str() || scope_header == required_scope.as_str() {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}
