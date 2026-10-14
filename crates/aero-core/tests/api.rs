//! Black-box API tests (same scenarios as the October 2026 test report).
//!
//! Tests marked `#[ignore = "Phase …"]` describe required behaviour that is not implemented yet;
//! they document the gap and must be un-ignored when that phase lands. Run them with
//! `cargo test -- --ignored` to see the current state.

use std::sync::Arc;

use aero_config::AeroConfig;
use aero_core::{router::build_router, AeroApp};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn app() -> Router {
    let mut cfg = AeroConfig::default();
    cfg.security.jwt_secret = "test-secret-test-secret-test-secret-123456".into();
    let app = Arc::new(AeroApp::new(cfg).await.expect("app"));
    build_router(&app)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, v)
}

async fn register(app: &Router, email: &str) -> String {
    let (s, v) = call(
        app,
        "POST",
        "/v1/auth/register",
        None,
        Some(json!({"email": email, "password": "password123", "name": email})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "register {email}: {v}");
    v["token"].as_str().unwrap().to_string()
}

/// Owner A with workspace; returns (app, token_a, workspace_id)
async fn with_workspace() -> (Router, String, String) {
    let app = app().await;
    let ta = register(&app, "a@x.io").await;
    let (s, ws) = call(
        &app,
        "POST",
        "/v1/workspaces",
        Some(&ta),
        Some(json!({"name": "Acme", "slug": "acme"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{ws}");
    (app, ta, ws["id"].as_str().unwrap().to_string())
}

// ---------------------------------------------------------------- works today

#[tokio::test]
async fn health() {
    let app = app().await;
    let (s, _) = call(&app, "GET", "/health", None, None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn register_login_me_refresh() {
    let app = app().await;
    let (s, a) = call(
        &app,
        "POST",
        "/v1/auth/register",
        None,
        Some(json!({"email":"a@x.io","password":"password123","name":"Alice"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let token = a["token"].as_str().unwrap();

    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({"email":"a@x.io","password":"wrong"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, v) = call(
        &app,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({"email":"a@x.io","password":"password123"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["token"].is_string());

    let (s, me) = call(&app, "GET", "/v1/auth/me", Some(token), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["email"], "a@x.io");

    let tampered = format!("{}abc", &token[..token.len() - 3]);
    let (s, _) = call(&app, "GET", "/v1/auth/me", Some(&tampered), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let (s, v) = call(
        &app,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({"refresh_token": a["refresh_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn workspaces_switch_invite_accept() {
    let (app, ta, wid) = with_workspace().await;
    let (s, list) = call(&app, "GET", "/v1/workspaces", Some(&ta), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(list.as_array().unwrap().iter().any(|w| w["id"] == wid));

    let (s, v) = call(
        &app,
        "POST",
        "/v1/auth/switch-workspace",
        Some(&ta),
        Some(json!({"workspace_id": wid})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v["token"].is_string());

    let (s, inv) = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{wid}/invite"),
        Some(&ta),
        Some(json!({"email":"b@x.io","role":"manager"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{inv}");
    let tb = register(&app, "b@x.io").await;
    let token = inv["invite_token"]
        .as_str()
        .or_else(|| inv["token"].as_str())
        .expect("invite token");
    let (s, v) = call(
        &app,
        "POST",
        "/v1/workspaces/invite/accept",
        Some(&tb),
        Some(json!({"invite_token": token})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, m) = call(
        &app,
        "GET",
        &format!("/v1/workspaces/{wid}/members"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(m["members"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn api_key_create_and_verify() {
    let (app, ta, _) = with_workspace().await;
    let (s, k) = call(
        &app,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"k1","scopes":["functions_read"],"credits_limit":100})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{k}");
    let (_, v) = call(
        &app,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": k["key"]})),
    )
    .await;
    assert_eq!(v["valid"], true);
    let (_, v) = call(
        &app,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": "aero_fake"})),
    )
    .await;
    assert_eq!(v["valid"], false);
}

#[tokio::test]
async fn dependency_scan_finds_known_cve() {
    let app = app().await;
    let (s, v) = call(
        &app,
        "POST",
        "/v1/remediation/scan",
        None,
        Some(json!({"manifest_type":"cargo_toml","manifest_content":"[dependencies]\njsonwebtoken = \"8.5.1\"\n"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["critical_count"], 1);
}

// ---------------------------------------------------------------- Phase 1: security

#[tokio::test]
#[ignore = "Phase 1: workspace routes must check membership"]
async fn outsider_cannot_read_or_delete_workspace() {
    let (app, _, wid) = with_workspace().await;
    let tc = register(&app, "c@x.io").await;
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/workspaces/{wid}"),
        Some(&tc),
        None,
    )
    .await;
    assert!(matches!(s, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND));
    let (s, _) = call(&app, "GET", &format!("/v1/workspaces/{wid}"), None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &app,
        "GET",
        &format!("/v1/workspaces/{wid}/members"),
        Some(&tc),
        None,
    )
    .await;
    assert!(matches!(s, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND));
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/v1/workspaces/{wid}"),
        Some(&tc),
        None,
    )
    .await;
    assert!(matches!(s, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND));
}

#[tokio::test]
#[ignore = "Phase 1: keys routes must require auth and be scoped to the caller's workspace"]
async fn keys_require_auth_and_are_isolated() {
    let (app, ta, _) = with_workspace().await;
    let (s, _) = call(
        &app,
        "POST",
        "/v1/keys",
        None,
        Some(json!({"name":"x","scopes":["admin"]})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(&app, "GET", "/v1/keys", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    call(
        &app,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"a-key","scopes":["functions_read"]})),
    )
    .await;
    let tc = register(&app, "c@x.io").await;
    let (_, list) = call(&app, "GET", "/v1/keys", Some(&tc), None).await;
    assert!(
        list.as_array().map(|l| l.is_empty()).unwrap_or(true),
        "outsider sees keys: {list}"
    );
}

#[tokio::test]
#[ignore = "Phase 1: function routes must require auth"]
async fn functions_require_auth() {
    let app = app().await;
    let (s, _) = call(
        &app,
        "POST",
        "/v1/functions",
        None,
        Some(json!({"name":"f","wasm_bytes":[0,97,115,109,1,0,0,0],"entrypoint":"main"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(&app, "GET", "/v1/functions", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "Phase 1: audit log and traces must require auth"]
async fn audit_and_traces_require_auth() {
    let app = app().await;
    for p in [
        "/v1/audit/recent",
        "/v1/audit/events",
        "/v1/inspector/traces",
    ] {
        let (s, _) = call(&app, "GET", p, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{p}");
    }
}

#[tokio::test]
#[ignore = "Phase 1: input validation and correct status codes"]
async fn validation_and_status_codes() {
    let app = app().await;
    register(&app, "a@x.io").await;
    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/register",
        None,
        Some(json!({"email":"a@x.io","password":"password123","name":"Dup"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &app,
        "POST",
        "/v1/auth/register",
        None,
        Some(json!({"email":"w@x.io","password":"1","name":"W"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(&app, "GET", "/v1/auth/me", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "Phase 1: request inspector must record requests"]
async fn inspector_records_requests() {
    let app = app().await;
    call(&app, "GET", "/health", None, None).await;
    let (_, v) = call(&app, "GET", "/v1/inspector/stats", None, None).await;
    assert!(v["total_requests"].as_u64().unwrap_or(0) > 0, "{v}");
}

// ---------------------------------------------------------------- Phase 2: real runtime

#[tokio::test]
#[ignore = "Phase 2: Wasm deploy must validate the module"]
async fn invalid_wasm_is_rejected() {
    let (app, ta, _) = with_workspace().await;
    let (s, _) = call(
        &app,
        "POST",
        "/v1/functions",
        Some(&ta),
        Some(json!({"name":"junk","wasm_bytes":[1,2,3],"entrypoint":"main"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "Phase 2: AI must call a real model (Ollama / OpenAI-compatible)"]
async fn ai_returns_model_output() {
    let (app, ta, _) = with_workspace().await;
    let (_, v) = call(
        &app,
        "POST",
        "/v1/ai/generate",
        Some(&ta),
        Some(json!({"prompt":"Say OK"})),
    )
    .await;
    assert!(!v.to_string().contains("Processed prompt"), "{v}");
}
