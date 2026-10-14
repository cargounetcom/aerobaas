# AeroBaaS roadmap

Decisions (14 Oct 2026): **PostgreSQL**, **Rust only** (Erlang orchestrator retired to `legacy/`), **closed commercial licence**, **SovereignBaaS Admin is the default admin UI**.

Each phase ends with its tests green. Gaps are tracked as `#[ignore = "Phase …"]` tests in `crates/aero-core/tests/api.rs`; un-ignore them as the phase lands.

## Phase 0 — honest and buildable ✅ (week 1)
- [x] Compiles; `cargo fmt`, `clippy -D warnings`, tests in CI; image published to GHCR
- [x] Configuration from `AERO_*` environment variables; `AERO_JWT_SECRET` required (was hardcoded)
- [x] API test suite: 5 passing scenarios + 8 documented gaps
- [x] README states what is real and what is a stub; proprietary licence

## Phase 1 — security and data (weeks 2–3)
- [ ] Auth + membership/role check on every route (workspaces, members, keys, functions, chat, audit, inspector)
- [ ] Keys and functions scoped to the caller's workspace
- [ ] Argon2id password hashing; registration validation; 409/400/401 status codes
- [ ] Secrets only from env (compliance HMAC key, encryption key); CORS allow-list; login rate limit
- [ ] PostgreSQL via sqlx with migrations; data survives restart
- [ ] Request inspector and audit log record every request
- [ ] **Admin-compatible API** (`/api/me`, `/api/workspaces/{ws}/members|invites|inspector|ai`) so the SovereignBaaS Admin works unchanged
- Gate: all `Phase 1` tests un-ignored and green; SovereignBaaS admin e2e suite passes against AeroBaaS

## Phase 2 — the core value (weeks 3–5)
- [ ] Wasm runtime on Wasmtime: module validation, memory/CPU/time limits, egress allow-list, cold-start benchmark
- [ ] AI via OpenAI-compatible endpoint (Ollama, OpenRouter, Gemini, MiMo) with streaming; embedded Candle later as an option
- [ ] MCP server: JSON-RPC over WebSocket, tools: read schema, create table, deploy/run function, query logs; per-agent key scopes
- Gate: an agent connects over MCP, creates a table, deploys and runs a function

## Phase 3 — product shell (weeks 5–6)
- [ ] SovereignBaaS Admin `engine: aero` adapter + AeroBaaS-only pages (Functions, Agent keys, Sandbox metrics, Dependency scan)
- [ ] CLI `aero init | deploy | keys`; quick start docs
- [ ] Installer (based on the SovereignBaaS installer): one command, VPS, HTTPS

## Phase 4 — scale (weeks 6–7)
- [ ] Agent locks/queues/credits in Rust (tokio + Postgres advisory locks) replacing the Erlang plan
- [ ] Load test; realistic target 5–10k req/s on 4 vCPU

## Phase 5 — launch (week 8)
- [ ] Private beta (own products first), licence keys `AERO-…` via the unified licensing service, website section, security review
