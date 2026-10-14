# AeroBaaS — agent-native backend

A backend for AI agents: workspaces and roles, scoped API keys, sandboxed functions, a local/cloud LLM and an MCP endpoint, in one Rust binary. Proprietary — © Aurora Ember Cyber Bio Lab (see `LICENSE`).

The admin UI is the **SovereignBaaS Admin** (engine adapter in Phase 3). Plan and status: **[ROADMAP.md](ROADMAP.md)**.

## What is real today (Phase 2a)

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
| Wasm functions | ✅ Wasmtime sandbox: WASI preview 1 modules, checked at deploy, memory/CPU/time limits, no files or network; execution history |
| Tables | ✅ per-workspace tables with typed columns, validated rows (JSON in PostgreSQL), filters |
| MCP for AI agents | ✅ `POST /v1/mcp` (Streamable HTTP): 8 tools — tables, functions, logs — limited by the key's scopes |
| AI generate / chat | 🟡 stub text (real models in Phase 2b); chat history is in memory |
| Sandbox metrics | 🟡 endpoints exist (platform admins only), not fed yet (Phase 3) |

## Run

```bash
cp .env.example .env            # set AERO_JWT_SECRET (openssl rand -base64 48) and POSTGRES_PASSWORD
docker compose up -d            # PostgreSQL + AeroBaaS → http://localhost:8080/health
```

Without Docker: start PostgreSQL, set `AERO_DATABASE_URL` and `AERO_JWT_SECRET`, then `cargo run --release --bin aero -- serve`.

## Functions

A function is a WASI preview 1 module (e.g. Rust built with `--target wasm32-wasip1`): it reads its JSON payload from **stdin**, writes the result to **stdout**, and anything on **stderr** comes back as `logs`. Example: `examples/functions/hello-rust`.

```bash
cargo build --release --target wasm32-wasip1        # in your function's folder
curl -X POST $AERO_URL/v1/functions -H "Authorization: Bearer $AERO_TOKEN" -H 'content-type: application/json' \
  -d "{\"name\":\"hello\",\"wasm_base64\":\"$(base64 -w0 target/wasm32-wasip1/release/hello-rust.wasm)\"}"
curl -X POST $AERO_URL/v1/functions/<id>/execute -H "Authorization: Bearer $AERO_TOKEN" -H 'content-type: application/json' \
  -d '{"payload":{"name":"Aurora"}}'
```

Limits per function: memory (`memory_limit_mb`, default 128), time (`timeout_ms`, default 30 s) and a CPU budget tied to the time limit. A stopped run answers **422** with `status` = `timeout`, `out_of_fuel`, `out_of_memory` or `error`. Measured on the test machine with a small module: about 8 ms for the first run (compile) and about 0.1–0.2 ms per run after that (`cargo test --release -p aero-wasm-runtime timings -- --nocapture`).

## MCP (for AI agents)

Create a key with `mcp_connect` plus the scopes the agent may use (`databases_read`, `databases_write`, `functions_list`, `functions_write`, `executions_write`, `admin` for logs). Then add the server to the agent, for example:

```json
{ "mcpServers": { "aerobaas": { "type": "http", "url": "https://YOUR-HOST/v1/mcp",
  "headers": { "Authorization": "Bearer aero_..." } } } }
```

Tools: `list_tables`, `create_table`, `insert_rows`, `query_rows`, `list_functions`, `deploy_function`, `run_function`, `query_logs`. Every call costs one credit and is recorded in the audit log as an AI-agent action.

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
