# AeroBaaS — agent-native backend

A backend for AI agents: workspaces and roles, scoped API keys, sandboxed functions, a local/cloud LLM and an MCP endpoint, in one Rust binary. Proprietary — © Aurora Ember Cyber Bio Lab (see `LICENSE`).

The admin UI is the **SovereignBaaS Admin** (engine adapter in Phase 3). Plan and status: **[ROADMAP.md](ROADMAP.md)**.

## What is real today (Phase 1)

| Area | Status |
|---|---|
| Sign-up, login, JWT access + refresh tokens, `/me` | ✅ Argon2id passwords, validation, login lock after repeated failures |
| Workspaces, members, roles, invites | ✅ membership and role checked on every request (Owner > Admin > Manager > User, same rules as the SovereignBaaS admin) |
| Scoped API keys (Keys API 2.0) | ✅ per-workspace, scopes enforced, function allow-lists, credits (1 per request; the key turns off at 0), expiry |
| Storage | ✅ PostgreSQL; schema migrations run at startup |
| Request inspector | ✅ every request recorded (no bodies, no query strings, invite tokens removed); 30-day retention |
| Audit log | ✅ sign-ins, workspace, member, key, function and scan events; live stream over WebSocket |
| SovereignBaaS admin API (`/api/*`) | ✅ me, workspaces, members, invites, inspector — see below |
| Dependency scan (package.json, Cargo.toml, go.mod, pyproject) | ✅ against a small built-in CVE list |
| Wasm functions | 🟡 modules stored per workspace; execute returns a placeholder (Wasmtime in Phase 2) |
| AI generate / chat | 🟡 stub text (real models in Phase 2); chat history is in memory |
| MCP | 🟡 WebSocket accepted for authorised callers, protocol not implemented (Phase 2) |
| Sandbox metrics | 🟡 endpoints exist (platform admins only), not fed yet (Phase 3) |

## Run

```bash
cp .env.example .env            # set AERO_JWT_SECRET (openssl rand -base64 48) and POSTGRES_PASSWORD
docker compose up -d            # PostgreSQL + AeroBaaS → http://localhost:8080/health
```

Without Docker: start PostgreSQL, set `AERO_DATABASE_URL` and `AERO_JWT_SECRET`, then `cargo run --release --bin aero -- serve`.

## Authentication

* People: `POST /v1/auth/login` → `Authorization: Bearer <token>`. The token carries the active workspace; change it with `POST /v1/auth/switch-workspace` or send `X-Aero-Workspace: <id>`.
* Agents: `Authorization: Bearer aero_…` or `X-Aero-Key: aero_…`. A key works only in its own workspace and only for its scopes.
* Public routes: `/health`, `/v1/auth/register|login|refresh`, `/v1/keys/verify`, `GET /api/invites/{token}`.

## SovereignBaaS admin API

| Route | Who |
|---|---|
| `GET /api/me`, `GET /api/platform/me` | signed in |
| `GET/POST /api/workspaces` | signed in |
| `GET /api/workspaces/{ws}/me` · `GET …/members` · `GET …/inspector/stats` | members |
| `GET/POST …/invites` · `DELETE …/invites/{id}` · `GET …/inspector/requests` | Manager+ |
| `PATCH/DELETE …/members/{membershipId}` | by role rules |
| `GET /api/invites/{token}` · `POST /api/invites/{token}/accept` | public · signed in with the invited email |

`/api/workspaces/{ws}/ai/*` arrives in Phase 2. The admin's sign-in screen needs the `engine: aero` adapter (Phase 3).

## Develop

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
export AERO_TEST_DATABASE_URL=postgres://aero:aero@127.0.0.1/aero   # any PostgreSQL; each test uses its own schema
cargo test --workspace                                   # must be green
cargo test -p aero-core --test api -- --ignored          # roadmap gaps (expected to fail until their phase)
```

## Layout

```
crates/
  aero-core          Axum server: routes (router.rs, admin_api.rs), auth extractors (auth.rs),
                     request inspector (inspector.rs), audit log (audit.rs), tests (tests/api.rs)
  aero-cli           `aero` binary: serve, create-key, init, scan, deploy
  aero-auth          JWT, register/login, workspace switch
  aero-tenancy       workspaces, members, roles, invites
  aero-keys          scoped API keys and credits
  aero-db            PostgreSQL pool, migrations (crates/aero-db/migrations), keys + functions
  aero-wasm-runtime  function sandbox (stub until Phase 2)
  aero-ai-engine     AI inference + content validation (stub until Phase 2)
  aero-mcp-server    MCP tools (Phase 2)
  aero-remediation   dependency CVE scan
  aero-chat, aero-realtime, aero-metrics, aero-skills, aero-crypto, aero-config, aero-protocol
openapi/aerobaas.yaml   API description
legacy/                 frontend prototype and Erlang orchestrator (retired; see ROADMAP)
```
