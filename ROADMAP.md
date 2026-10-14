# AeroBaaS roadmap

Decisions (14 Oct 2026): **PostgreSQL**, **Rust only** (Erlang orchestrator retired to `legacy/`), **closed commercial licence**, **SovereignBaaS Admin is the default admin UI**.

Each phase ends with its tests green. Gaps are tracked as `#[ignore = "Phase …"]` tests in `crates/aero-core/tests/api.rs`; un-ignore them as the phase lands.

## Phase 0 — honest and buildable ✅ (week 1)
- [x] Compiles; `cargo fmt`, `clippy -D warnings`, tests in CI; image published to GHCR
- [x] Configuration from `AERO_*` environment variables; `AERO_JWT_SECRET` required (was hardcoded)
- [x] API test suite: 5 passing scenarios + 8 documented gaps
- [x] README states what is real and what is a stub; proprietary licence

## Phase 1 — security and data ✅ (14 Oct 2026)
- [x] Auth + membership/role check on every route (workspaces, members, keys, functions, chat, audit, inspector, metrics)
- [x] Keys and functions scoped to the caller's workspace; key scopes, function allow-lists, credits and expiry enforced (IDEAS B1, B2)
- [x] Argon2id password hashing; registration validation; 400/401/403/404/409/429 status codes; JSON errors `{error, message, status}`
- [x] Secrets only from env (JWT, compliance signing key); CORS allow-list; login lock (per email and per IP)
- [x] PostgreSQL via sqlx with migrations; data survives restart (tested)
- [x] Request inspector records every request; audit log records sign-ins and every change (IDEAS G2, storage part)
- [x] **Admin-compatible API**: `/api/me`, `/api/workspaces`, `/api/workspaces/{ws}/me|members|invites|inspector/*`, `/api/invites/{token}`
- Gate met: 21 API tests green on PostgreSQL in CI (`crates/aero-core/tests/api.rs`)
- Moved: `/api/workspaces/{ws}/ai/*` → Phase 2 (needs the real AI layer); running the SovereignBaaS admin e2e suite → Phase 3 (needs the `engine: aero` sign-in adapter)
- Later: refresh-token revocation (sign-out everywhere), email verification, password reset

## Phase 2a — functions and MCP ✅ (14 Oct 2026)
- [x] Wasm runtime on Wasmtime 49: module checked at deploy (valid Wasm, WASI preview 1 imports only, `() -> ()` entrypoint), memory/CPU (fuel)/wall-clock limits, no files, env or network; compiled modules cached; execution history (IDEAS A1, A2, D1, D2)
- [x] Cold/warm start measured (small module: ~8 ms first run, ~0.1–0.2 ms after) — `timings` test
- [x] Tables: typed columns, validated JSON rows, equality filters; key allow-lists (`allowed_database_ids`)
- [x] MCP server: Streamable HTTP (`POST /v1/mcp`), 8 tools, per-key scopes, audited as AI-agent actions (IDEAS C1)
- Gate met: `mcp_agent_end_to_end` — an agent creates a table, inserts and queries rows, deploys and runs a function
- Later: host HTTP for functions with a per-function egress allow-list; environment variables/secrets for functions; larger-module benchmark

## Phase 2b — AI (next)
- [ ] AI via OpenAI-compatible endpoint (Ollama, OpenRouter, Gemini, MiMo) with streaming; embedded Candle later as an option
- [ ] Admin AI routes `/api/workspaces/{ws}/ai/providers|models|chat` (encrypted provider keys, same shapes as SovereignBaaS); chat history in PostgreSQL
- [ ] `Aero.ai.generateText()` host function for Wasm (IDEAS E2)

## Phase 3 — product shell (weeks 5–6)
- [ ] SovereignBaaS Admin `engine: aero` adapter (sign-in against AeroBaaS) + AeroBaaS-only pages (Functions, Agent keys, Sandbox metrics, Dependency scan, live audit log)
- [ ] Gate: the SovereignBaaS admin e2e suite passes against AeroBaaS
- [ ] CLI `aero init | deploy | keys`; quick start docs
- [ ] Installer (based on the SovereignBaaS installer): one command, VPS, HTTPS

## Phase 4 — scale (weeks 6–7)
- [ ] Agent locks/queues/credits in Rust (tokio + Postgres advisory locks) replacing the Erlang plan
- [ ] Load test; realistic target 5–10k req/s on 4 vCPU

## Phase 5 — launch (week 8)
- [ ] Private beta (own products first), licence keys `AERO-…` via the unified licensing service, website section, security review
