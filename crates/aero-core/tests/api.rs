//! Black-box API tests against a real PostgreSQL.
//!
//! Set `AERO_TEST_DATABASE_URL` (e.g. `postgres://aero:aero@127.0.0.1/aero`). Each test runs in
//! its own schema. Without the variable the tests are skipped locally and fail in CI.
//!
//! Tests marked `#[ignore = "Phase …"]` describe required behaviour that is not implemented yet;
//! un-ignore them when that phase lands. `cargo test -- --ignored` shows the open gaps.

use std::sync::Arc;
use std::time::Duration;

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

struct T {
    router: Router,
    cfg: AeroConfig,
}

async fn config() -> Option<AeroConfig> {
    let Ok(base) = std::env::var("AERO_TEST_DATABASE_URL") else {
        if std::env::var("CI").is_ok() {
            panic!("AERO_TEST_DATABASE_URL must be set in CI");
        }
        eprintln!("skipped: set AERO_TEST_DATABASE_URL to run the API tests");
        return None;
    };
    let schema = format!("t_{}", uuid::Uuid::new_v4().simple());
    let pool = sqlx::PgPool::connect(&base).await.expect("test database");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&pool)
        .await
        .expect("create schema");
    pool.close().await;
    let sep = if base.contains('?') { '&' } else { '?' };
    let mut cfg = AeroConfig::default();
    cfg.security.jwt_secret = "test-secret-test-secret-test-secret-123456".into();
    cfg.database.url = format!("{base}{sep}options=-c%20search_path%3D{schema}");
    cfg.database.max_connections = 4;
    Some(cfg)
}

async fn start(cfg: AeroConfig) -> T {
    let app = Arc::new(AeroApp::new(cfg.clone()).await.expect("app"));
    T {
        router: build_router(&app),
        cfg,
    }
}

async fn app() -> Option<T> {
    Some(start(config().await?).await)
}

async fn req(
    t: &T,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(tk) = token {
        b = b.header("authorization", format!("Bearer {tk}"));
    }
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let r = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let res = t.router.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, v)
}

async fn call(
    t: &T,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    req(t, method, uri, token, body, &[]).await
}

/// Registers and returns (access token, personal workspace id).
async fn register_ws(t: &T, email: &str) -> (String, String) {
    let (s, v) = call(
        t,
        "POST",
        "/v1/auth/register",
        None,
        Some(json!({"email": email, "password": "password123", "name": email})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "register {email}: {v}");
    (
        v["token"].as_str().unwrap().to_string(),
        v["default_workspace"]["id"].as_str().unwrap().to_string(),
    )
}

async fn register(t: &T, email: &str) -> String {
    register_ws(t, email).await.0
}

/// Owner A with an extra workspace "acme"; returns (app, token_a, acme id)
async fn with_workspace() -> Option<(T, String, String)> {
    let t = app().await?;
    let ta = register(&t, "a@x.io").await;
    let (s, ws) = call(
        &t,
        "POST",
        "/v1/workspaces",
        Some(&ta),
        Some(json!({"name": "Acme", "slug": "acme"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{ws}");
    let id = ws["id"].as_str().unwrap().to_string();
    Some((t, ta, id))
}

/// Switch the token to workspace `ws`.
async fn switch(t: &T, token: &str, ws: &str) -> String {
    let (s, v) = call(
        t,
        "POST",
        "/v1/auth/switch-workspace",
        Some(token),
        Some(json!({"workspace_id": ws})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["token"].as_str().unwrap().to_string()
}

/// Invite `email` as `role` into `ws` (by `inviter`), register them and accept. Returns their token.
async fn add_member(t: &T, inviter: &str, ws: &str, email: &str, role: &str) -> String {
    let (s, inv) = call(
        t,
        "POST",
        &format!("/v1/workspaces/{ws}/invite"),
        Some(inviter),
        Some(json!({"email": email, "role": role})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{inv}");
    let tok = register(t, email).await;
    let (s, v) = call(
        t,
        "POST",
        "/v1/workspaces/invite/accept",
        Some(&tok),
        Some(json!({"invite_token": inv["invite_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    switch(t, &tok, ws).await
}

const WASM: [u8; 8] = [0, 97, 115, 109, 1, 0, 0, 0];

async fn deploy(t: &T, token: &str, name: &str) -> String {
    let (s, v) = call(
        t,
        "POST",
        "/v1/functions",
        Some(token),
        Some(json!({"name": name, "wasm_bytes": WASM, "entrypoint": "main"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["function_id"].as_str().unwrap().to_string()
}

/// Request rows are written in the background; wait until `check` passes.
async fn eventually<F, Fut>(mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..50 {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("condition not met in time");
}

// ---------------------------------------------------------------- basics

#[tokio::test]
async fn health() {
    let Some(t) = app().await else { return };
    let (s, _) = call(&t, "GET", "/health", None, None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn register_login_me_refresh() {
    let Some(t) = app().await else { return };
    let (s, a) = call(
        &t,
        "POST",
        "/v1/auth/register",
        None,
        Some(json!({"email":"A@X.io","password":"password123","name":"Alice"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(a["user"]["email"], "a@x.io", "email is normalised");
    let token = a["token"].as_str().unwrap();

    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({"email":"a@x.io","password":"wrong-password"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, v) = call(
        &t,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({"email":"a@x.io","password":"password123"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["default_workspace"]["id"], a["default_workspace"]["id"]);

    let (s, me) = call(&t, "GET", "/v1/auth/me", Some(token), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["email"], "a@x.io");

    let tampered = format!("{}abc", &token[..token.len() - 3]);
    let (s, _) = call(&t, "GET", "/v1/auth/me", Some(&tampered), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let refresh = a["refresh_token"].as_str().unwrap();
    let (s, _) = call(&t, "GET", "/v1/auth/me", Some(refresh), None).await;
    assert_eq!(
        s,
        StatusCode::UNAUTHORIZED,
        "refresh token is not an access token"
    );
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({"refresh_token": token})),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::UNAUTHORIZED,
        "access token is not a refresh token"
    );
    let (s, v) = call(
        &t,
        "POST",
        "/v1/auth/refresh",
        None,
        Some(json!({"refresh_token": refresh})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn workspaces_switch_invite_accept() {
    let Some((t, ta, wid)) = with_workspace().await else {
        return;
    };
    let (s, list) = call(&t, "GET", "/v1/workspaces", Some(&ta), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 2, "personal + acme: {list}");
    assert!(list.as_array().unwrap().iter().any(|w| w["id"] == wid));

    let (s, v) = call(
        &t,
        "POST",
        "/v1/auth/switch-workspace",
        Some(&ta),
        Some(json!({"workspace_id": wid})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["role"], "owner");
    assert_eq!(v["workspace"]["name"], "Acme");

    let tb = add_member(&t, &ta, &wid, "b@x.io", "manager").await;
    let (s, m) = call(
        &t,
        "GET",
        &format!("/v1/workspaces/{wid}/members"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(m["members"].as_array().unwrap().len(), 2);
    assert_eq!(m["total"], 2);
}

#[tokio::test]
async fn api_key_create_and_verify() {
    let Some((t, ta, _)) = with_workspace().await else {
        return;
    };
    let (s, k) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"k1","scopes":["functions_read"],"credits_limit":100})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{k}");
    assert!(k["key"].as_str().unwrap().starts_with("aero_"));
    let (_, v) = call(
        &t,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": k["key"]})),
    )
    .await;
    assert_eq!(v["valid"], true);
    assert_eq!(v["credits_remaining"], 100);
    let (_, v) = call(
        &t,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": "aero_fake"})),
    )
    .await;
    assert_eq!(v["valid"], false);

    let (_, list) = call(&t, "GET", "/v1/keys", Some(&ta), None).await;
    assert!(
        list[0].get("key_hash").is_none(),
        "hash must not leak: {list}"
    );
}

#[tokio::test]
async fn dependency_scan_finds_known_cve() {
    let Some(t) = app().await else { return };
    let ta = register(&t, "a@x.io").await;
    let (s, v) = call(
        &t,
        "POST",
        "/v1/remediation/scan",
        Some(&ta),
        Some(json!({"manifest_type":"cargo_toml","manifest_content":"[dependencies]\njsonwebtoken = \"8.5.1\"\n"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["critical_count"], 1);
    let (s, _) = call(
        &t,
        "POST",
        "/v1/remediation/scan",
        None,
        Some(json!({"manifest_type":"cargo_toml","manifest_content":""})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------- Phase 1: security

#[tokio::test]
async fn outsider_cannot_read_or_delete_workspace() {
    let Some((t, _, wid)) = with_workspace().await else {
        return;
    };
    let tc = register(&t, "c@x.io").await;
    for (m, p) in [
        ("GET", format!("/v1/workspaces/{wid}")),
        ("GET", format!("/v1/workspaces/{wid}/members")),
        ("DELETE", format!("/v1/workspaces/{wid}")),
        ("PUT", format!("/v1/workspaces/{wid}")),
    ] {
        let body = (m == "PUT").then(|| json!({"name": "pwned"}));
        let (s, _) = call(&t, m, &p, Some(&tc), body).await;
        assert!(
            matches!(s, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND),
            "{m} {p}: {s}"
        );
    }
    let (s, _) = call(&t, "GET", &format!("/v1/workspaces/{wid}"), None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn keys_require_auth_and_are_isolated() {
    let Some((t, ta, _)) = with_workspace().await else {
        return;
    };
    let (s, _) = call(
        &t,
        "POST",
        "/v1/keys",
        None,
        Some(json!({"name":"x","scopes":["admin"]})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(&t, "GET", "/v1/keys", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, k) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"a-key","scopes":["functions_read"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let tc = register(&t, "c@x.io").await;
    let (s, list) = call(&t, "GET", "/v1/keys", Some(&tc), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        list.as_array().unwrap().is_empty(),
        "outsider sees keys: {list}"
    );
    let (s, _) = call(
        &t,
        "GET",
        &format!("/v1/keys/{}", k["id"].as_str().unwrap()),
        Some(&tc),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        &t,
        "DELETE",
        &format!("/v1/keys/{}", k["id"].as_str().unwrap()),
        Some(&tc),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn functions_require_auth() {
    let Some(t) = app().await else { return };
    let (s, _) = call(
        &t,
        "POST",
        "/v1/functions",
        None,
        Some(json!({"name":"f","wasm_bytes":WASM,"entrypoint":"main"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(&t, "GET", "/v1/functions", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn functions_are_isolated_per_workspace() {
    let Some(t) = app().await else { return };
    let (ta, wa) = register_ws(&t, "a@x.io").await;
    let f = deploy(&t, &ta, "fa").await;
    let (_, list) = call(&t, "GET", "/v1/functions", Some(&ta), None).await;
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");

    let tc = register(&t, "c@x.io").await;
    let (s, _) = call(&t, "GET", &format!("/v1/functions/{f}"), Some(&tc), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = req(
        &t,
        "GET",
        "/v1/functions",
        Some(&tc),
        None,
        &[("x-aero-workspace", wa.as_str())],
    )
    .await;
    assert_eq!(
        s,
        StatusCode::FORBIDDEN,
        "header cannot select a foreign workspace"
    );
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/switch-workspace",
        Some(&tc),
        Some(json!({"workspace_id": wa})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (s, v) = call(
        &t,
        "POST",
        &format!("/v1/functions/{f}/execute"),
        Some(&ta),
        Some(json!({"payload": {}})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn key_scopes_allow_list_and_credits() {
    let Some(t) = app().await else { return };
    let ta = register(&t, "a@x.io").await;
    let f1 = deploy(&t, &ta, "one").await;
    let _f2 = deploy(&t, &ta, "two").await;

    let (_, lister) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"lister","scopes":["functions_list"],"allowed_function_ids":[f1]})),
    )
    .await;
    let lk = lister["key"].as_str().unwrap();
    let (s, list) = call(&t, "GET", "/v1/functions", Some(lk), None).await;
    assert_eq!(s, StatusCode::OK, "{list}");
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![f1.as_str()], "allow-list filters the list");
    let (s, _) = call(
        &t,
        "POST",
        &format!("/v1/functions/{f1}/execute"),
        Some(lk),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "no executions.write scope");
    let (s, _) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(lk),
        Some(json!({"name":"x","scopes":["admin"]})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "keys cannot create keys");

    let (_, runner) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"runner","scopes":["executions_write"],"credits_limit":2})),
    )
    .await;
    let rk = runner["key"].as_str().unwrap();
    for _ in 0..2 {
        let (s, v) = req(
            &t,
            "POST",
            &format!("/v1/functions/{f1}/execute"),
            None,
            Some(json!({"payload":{"a":1}})),
            &[("x-aero-key", rk)],
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
    }
    let (s, _) = call(
        &t,
        "POST",
        &format!("/v1/functions/{f1}/execute"),
        Some(rk),
        Some(json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "credits exhausted");
    let (_, v) = call(
        &t,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": rk})),
    )
    .await;
    assert_eq!(v["valid"], false);

    let (s, _) = call(&t, "POST", "/v1/keys", Some(&ta), Some(json!({"name":"bad","scopes":["functions_read"],"allowed_function_ids":[uuid::Uuid::new_v4()]}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "foreign function id");
}

#[tokio::test]
async fn expired_key_is_rejected() {
    let Some(t) = app().await else { return };
    let ta = register(&t, "a@x.io").await;
    let past = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    let (s, _) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"k","scopes":["functions_list"],"expires_at": past})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let soon = (chrono::Utc::now() + chrono::Duration::milliseconds(1500)).to_rfc3339();
    let (s, k) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"k","scopes":["functions_list"],"expires_at": soon})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{k}");
    let key = k["key"].as_str().unwrap();
    let (s, _) = call(&t, "GET", "/v1/functions", Some(key), None).await;
    assert_eq!(s, StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(1700)).await;
    let (s, _) = call(&t, "GET", "/v1/functions", Some(key), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (_, v) = call(
        &t,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": key})),
    )
    .await;
    assert_eq!(v["valid"], false);
}

#[tokio::test]
async fn role_rules() {
    let Some((t, ta, wid)) = with_workspace().await else {
        return;
    };
    let ta = switch(&t, &ta, &wid).await;
    let tb = add_member(&t, &ta, &wid, "b@x.io", "manager").await;

    let invite = |tok: String, email: &'static str, role: &'static str| {
        let t = &t;
        let wid = wid.clone();
        async move {
            call(
                t,
                "POST",
                &format!("/v1/workspaces/{wid}/invite"),
                Some(&tok),
                Some(json!({"email": email, "role": role})),
            )
            .await
            .0
        }
    };
    assert_eq!(
        invite(tb.clone(), "x@x.io", "admin").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(invite(tb.clone(), "x@x.io", "user").await, StatusCode::OK);
    assert_eq!(
        invite(ta.clone(), "b@x.io", "user").await,
        StatusCode::CONFLICT,
        "already a member"
    );

    let (_, members) = call(
        &t,
        "GET",
        &format!("/v1/workspaces/{wid}/members"),
        Some(&ta),
        None,
    )
    .await;
    let a_id = members["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "owner")
        .unwrap()["user_id"]
        .as_str()
        .unwrap()
        .to_string();
    let b_id = members["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "manager")
        .unwrap()["user_id"]
        .as_str()
        .unwrap()
        .to_string();

    let (s, _) = call(
        &t,
        "PUT",
        &format!("/v1/workspaces/{wid}/members/{a_id}"),
        Some(&tb),
        Some(json!({"role":"user"})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "manager cannot change roles");
    let (s, _) = call(
        &t,
        "PUT",
        &format!("/v1/workspaces/{wid}/members/{a_id}"),
        Some(&ta),
        Some(json!({"role":"admin"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "last owner");
    let (s, _) = call(
        &t,
        "DELETE",
        &format!("/v1/workspaces/{wid}/members/{a_id}"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "last owner cannot leave");
    let (s, _) = call(
        &t,
        "DELETE",
        &format!("/v1/workspaces/{wid}"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "only owners delete");
    let (s, v) = call(
        &t,
        "PUT",
        &format!("/v1/workspaces/{wid}/members/{b_id}"),
        Some(&ta),
        Some(json!({"role":"admin"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["role"], "admin");
    let (s, _) = call(
        &t,
        "PUT",
        &format!("/v1/workspaces/{wid}"),
        Some(&ta),
        Some(json!({"plan":"enterprise"})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "plan comes from the licence");
    let (s, _) = call(
        &t,
        "DELETE",
        &format!("/v1/workspaces/{wid}"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn invite_is_bound_to_its_email() {
    let Some((t, ta, wid)) = with_workspace().await else {
        return;
    };
    let (_, inv) = call(
        &t,
        "POST",
        &format!("/v1/workspaces/{wid}/invite"),
        Some(&ta),
        Some(json!({"email":"b@x.io","role":"user"})),
    )
    .await;
    let tc = register(&t, "c@x.io").await;
    let (s, _) = call(
        &t,
        "POST",
        "/v1/workspaces/invite/accept",
        Some(&tc),
        Some(json!({"invite_token": inv["invite_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let tb = register(&t, "b@x.io").await;
    let (s, _) = call(
        &t,
        "POST",
        "/v1/workspaces/invite/accept",
        Some(&tb),
        Some(json!({"invite_token": inv["invite_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &t,
        "POST",
        "/v1/workspaces/invite/accept",
        Some(&tb),
        Some(json!({"invite_token": inv["invite_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "used invite");
}

#[tokio::test]
async fn audit_and_traces_require_auth() {
    let Some(t) = app().await else { return };
    for p in [
        "/v1/audit/recent",
        "/v1/audit/events",
        "/v1/inspector/traces",
        "/v1/inspector/stats",
        "/v1/metrics/snapshot",
        "/v1/skills",
        "/v1/chat/panels",
    ] {
        let (s, _) = call(&t, "GET", p, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{p}");
    }
}

#[tokio::test]
async fn validation_and_status_codes() {
    let Some(t) = app().await else { return };
    register(&t, "a@x.io").await;
    let reg =
        |email: &str, pw: &str, name: &str| json!({"email": email, "password": pw, "name": name});
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/register",
        None,
        Some(reg("A@x.io", "password123", "Dup")),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/register",
        None,
        Some(reg("w@x.io", "1", "W")),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/register",
        None,
        Some(reg("not-an-email", "password123", "W")),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/register",
        None,
        Some(reg("n@x.io", "password123", "  ")),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, v) = call(&t, "GET", "/v1/auth/me", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(v["error"], "unauthorized");
    assert!(v["message"].is_string());

    let ta = register(&t, "b@x.io").await;
    let (s, _) = call(
        &t,
        "POST",
        "/v1/workspaces",
        Some(&ta),
        Some(json!({"name":"X","slug":"Bad Slug!"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    call(
        &t,
        "POST",
        "/v1/workspaces",
        Some(&ta),
        Some(json!({"name":"X","slug":"taken"})),
    )
    .await;
    let (s, _) = call(
        &t,
        "POST",
        "/v1/workspaces",
        Some(&ta),
        Some(json!({"name":"Y","slug":"taken"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn passwords_are_argon2_and_data_survives_restart() {
    let Some(cfg) = config().await else { return };
    let t = start(cfg.clone()).await;
    let ta = register(&t, "a@x.io").await;
    let (_, k) = call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"k","scopes":["functions_list"]})),
    )
    .await;
    let f = deploy(&t, &ta, "persisted").await;
    drop(t);

    let t2 = start(cfg.clone()).await;
    let (s, v) = call(
        &t2,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({"email":"a@x.io","password":"password123"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (_, v) = call(
        &t2,
        "POST",
        "/v1/keys/verify",
        None,
        Some(json!({"key": k["key"]})),
    )
    .await;
    assert_eq!(v["valid"], true);
    let (s, _) = call(
        &t2,
        "GET",
        &format!("/v1/functions/{f}"),
        Some(v["token"].as_str().unwrap_or(&ta)),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let pool = sqlx::PgPool::connect(&t2.cfg.database.url).await.unwrap();
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE email = 'a@x.io'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"), "{hash}");
}

#[tokio::test]
async fn login_is_rate_limited() {
    let Some(mut cfg) = config().await else {
        return;
    };
    cfg.security.login_max_failures = 3;
    let t = start(cfg).await;
    register(&t, "a@x.io").await;
    for _ in 0..3 {
        let (s, _) = call(
            &t,
            "POST",
            "/v1/auth/login",
            None,
            Some(json!({"email":"a@x.io","password":"wrong-password"})),
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (s, _) = call(
        &t,
        "POST",
        "/v1/auth/login",
        None,
        Some(json!({"email":"a@x.io","password":"password123"})),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn cors_is_off_unless_configured() {
    let Some(mut cfg) = config().await else {
        return;
    };
    let t = start(cfg.clone()).await;
    let r = Request::builder()
        .uri("/health")
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let res = t.router.clone().oneshot(r).await.unwrap();
    assert!(res.headers().get("access-control-allow-origin").is_none());

    cfg.server.cors_origins = vec!["https://admin.example".into()];
    let t = start(cfg).await;
    for (origin, allowed) in [
        ("https://admin.example", true),
        ("https://evil.example", false),
    ] {
        let r = Request::builder()
            .method("OPTIONS")
            .uri("/v1/functions")
            .header("origin", origin)
            .header("access-control-request-method", "GET")
            .body(Body::empty())
            .unwrap();
        let res = t.router.clone().oneshot(r).await.unwrap();
        assert_eq!(
            res.headers().get("access-control-allow-origin").is_some(),
            allowed,
            "{origin}"
        );
    }
}

#[tokio::test]
async fn inspector_records_requests() {
    let Some(t) = app().await else { return };
    let ta = register(&t, "a@x.io").await;
    call(&t, "GET", "/v1/functions", Some(&ta), None).await;
    call(
        &t,
        "GET",
        "/v1/keys/00000000-0000-0000-0000-000000000000",
        Some(&ta),
        None,
    )
    .await;
    eventually(|| async {
        let (_, v) = call(&t, "GET", "/v1/inspector/stats", Some(&ta), None).await;
        v["total_requests"].as_u64().unwrap_or(0) >= 2
            && v["total_errors"].as_u64().unwrap_or(0) >= 1
    })
    .await;
    let (s, traces) = call(&t, "GET", "/v1/inspector/traces?limit=50", Some(&ta), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        traces
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["path"] == "/v1/functions"),
        "{traces}"
    );
}

#[tokio::test]
async fn audit_log_records_actions() {
    let Some((t, ta, wid)) = with_workspace().await else {
        return;
    };
    let ta = switch(&t, &ta, &wid).await;
    call(
        &t,
        "POST",
        "/v1/keys",
        Some(&ta),
        Some(json!({"name":"k","scopes":["functions_list"]})),
    )
    .await;
    let (s, events) = call(&t, "GET", "/v1/audit/recent", Some(&ta), None).await;
    assert_eq!(s, StatusCode::OK);
    let actions: Vec<&str> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    for a in ["key_created", "workspace_switched", "workspace_created"] {
        assert!(actions.contains(&a), "{a} missing in {actions:?}");
    }
    let (s, page) = call(
        &t,
        "GET",
        "/v1/audit/events?action=key_created",
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);

    let tu = add_member(&t, &ta, &wid, "u@x.io", "user").await;
    let (s, _) = call(&t, "GET", "/v1/audit/recent", Some(&tu), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "users cannot read the audit log");
}

// ---------------------------------------------------------------- SovereignBaaS admin contract

#[tokio::test]
async fn admin_api_contract() {
    let Some((t, ta, wid)) = with_workspace().await else {
        return;
    };

    let (s, me) = call(&t, "GET", "/api/me", Some(&ta), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["email"], "a@x.io");
    let (_, p) = call(&t, "GET", "/api/platform/me", Some(&ta), None).await;
    assert_eq!(p["platformAdmin"], true, "first user is platform admin");
    assert_eq!(p["edition"], "aero");

    let (_, list) = call(&t, "GET", "/api/workspaces", Some(&ta), None).await;
    assert_eq!(list["workspaces"].as_array().unwrap().len(), 2);
    let (s, created) = call(
        &t,
        "POST",
        "/api/workspaces",
        Some(&ta),
        Some(json!({"name":"Team Ünï"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{created}");
    assert!(created["slug"].as_str().unwrap().starts_with("team-"));

    let (s, acc) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/me"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(acc["role"], "owner");
    assert_eq!(acc["permissions"]["deleteWorkspace"], true);
    assert_eq!(
        acc["permissions"]["inviteRoles"].as_array().unwrap().len(),
        4
    );

    let (s, inv) = call(
        &t,
        "POST",
        &format!("/api/workspaces/{wid}/invites"),
        Some(&ta),
        Some(json!({"email":"B@x.io","role":"user"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{inv}");
    let link = inv["link"].as_str().unwrap();
    let token = link.rsplit('/').next().unwrap();
    assert!(link.contains("/invite/"));
    let (_, open) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/invites"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(open["invites"].as_array().unwrap().len(), 1);

    let (s, info) = call(&t, "GET", &format!("/api/invites/{token}"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(info["status"], "open");
    assert_eq!(info["workspaceName"], "Acme");

    let tb = register(&t, "b@x.io").await;
    let (s, joined) = call(
        &t,
        "POST",
        &format!("/api/invites/{token}/accept"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{joined}");
    assert_eq!(joined["role"], "user");

    let (_, members) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/members"),
        Some(&tb),
        None,
    )
    .await;
    let rows = members["members"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["role"], "owner", "sorted by role");
    let b_membership = rows.iter().find(|m| m["email"] == "b@x.io").unwrap()["membershipId"]
        .as_str()
        .unwrap()
        .to_string();

    let (s, _) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/inspector/requests"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "users cannot open the inspector");
    let (s, _) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/inspector/stats"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "every member sees stats");

    let (s, v) = call(
        &t,
        "PATCH",
        &format!("/api/workspaces/{wid}/members/{b_membership}"),
        Some(&ta),
        Some(json!({"role":"manager"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["role"], "manager");

    eventually(|| async {
        let (_, r) = call(
            &t,
            "GET",
            &format!("/api/workspaces/{wid}/inspector/requests?limit=100"),
            Some(&ta),
            None,
        )
        .await;
        r["items"].as_array().is_some_and(|i| {
            i.iter()
                .any(|x| x["path"] == format!("/api/workspaces/{wid}/members"))
        })
    })
    .await;
    let (_, r) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/inspector/requests?status=errors"),
        Some(&ta),
        None,
    )
    .await;
    assert!(r["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["status"].as_i64().unwrap() >= 400));
    let (_, all) = call(
        &t,
        "GET",
        "/api/workspaces/00000000-0000-0000-0000-000000000000/inspector/requests",
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(all["error"], "forbidden");
    let (_, st) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/inspector/stats"),
        Some(&ta),
        None,
    )
    .await;
    for k in [
        "requests24h",
        "errors24h",
        "aiRequests24h",
        "avgLatencyMs",
        "activeUsers24h",
        "members",
        "hourly",
    ] {
        assert!(st.get(k).is_some(), "{k} missing: {st}");
    }
    assert_eq!(st["members"], 2);
    let (_, rows) = call(
        &t,
        "GET",
        &format!("/api/workspaces/{wid}/inspector/requests"),
        Some(&ta),
        None,
    )
    .await;
    assert!(
        rows["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|x| !x["path"].as_str().unwrap().contains(token)),
        "invite tokens are never stored"
    );

    let (s, _) = call(
        &t,
        "DELETE",
        &format!("/api/workspaces/{wid}/members/{b_membership}"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

// ---------------------------------------------------------------- Phase 2: real runtime

#[tokio::test]
#[ignore = "Phase 2: Wasm deploy must validate the module"]
async fn invalid_wasm_is_rejected() {
    let Some((t, ta, _)) = with_workspace().await else {
        return;
    };
    let (s, _) = call(
        &t,
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
    let Some((t, ta, _)) = with_workspace().await else {
        return;
    };
    let (_, v) = call(
        &t,
        "POST",
        "/v1/ai/generate",
        Some(&ta),
        Some(json!({"prompt":"Say OK"})),
    )
    .await;
    assert!(!v.to_string().contains("Processed prompt"), "{v}");
}
