# Progress log

Newest first. One entry per working day: what was done (with proof), what is open, and the plan for the next day.

## 14 Oct 2026

### Done
| Area | Result | Proof |
|---|---|---|
| Phase 0 | Builds, fmt/clippy clean, proprietary licence, honest README, CI, GHCR image | CI green |
| Idea library | Original specs in `docs/source/`; `docs/IDEAS.md` maps every idea to a decision and phase; `CLAUDE.md` project prompt | — |
| Phase 1: security + data | PostgreSQL with migrations; Argon2id; auth and role checks on every route; workspace-scoped keys with scopes, allow-lists, credits and expiry; request inspector; audit log; SovereignBaaS admin API (`/api/*`) | 21 API tests on PostgreSQL |
| Phase 2a: functions + MCP | Wasmtime sandbox (WASI p1, memory/CPU/time limits, no files or network); execution history; tables with typed columns; MCP server (`POST /v1/mcp`, 8 tools) | 28 API tests incl. `mcp_agent_end_to_end`; Rust example built and run in CI |
| Speed (small module) | ~8 ms first run, ~0.1–0.2 ms warm | `timings` test |
| Online | AeroBaaS + its own PostgreSQL on Railway: `https://aerobaas-production.up.railway.app` (`/v1/info`, `/v1/mcp`) | Railway deploy SUCCESS, migrations applied |
| Admin | SovereignBaaS admin → **Manage → Agent keys**: AeroBaaS sign-in, generate keys (presets, credits, expiry), key shown once with MCP JSON and the Claude Code command, delete | Browser test on a local copy: key works as an MCP server, deleted key is refused, no errors |

### Open
* The Agent keys page has not yet been opened on Railway by a person (the build environment cannot reach railway.app). First check tomorrow.
* GitHub reports 3 Dependabot alerts (1 high, 2 moderate); not readable with the current token. Probably `legacy/frontend`.
* AI is still a stub (Phase 2b).
* SovereignBaaS: Hetzner installer test (owner), website placeholders (IBAN, emails, domain), SMTP.

### Plan for 15 Oct 2026
1. **Check online (15 min):** owner opens Admin → Agent keys on Railway, creates an AeroBaaS account and a key, sends a screenshot; Claude reads the Railway logs and fixes anything that fails.
2. **Connect a real agent (15 min):** paste the MCP config into Claude Code or Cursor; ask the agent to create a table and run the example function.
3. **Security housekeeping (30 min):** owner pastes the Dependabot list (GitHub → Security → Dependabot); remove `legacy/frontend` dependencies or update them.
4. **Phase 2b — AI (main task):** OpenAI-compatible providers (Ollama, OpenRouter, Gemini, MiMo) with encrypted keys per workspace; streaming chat; admin routes `/api/workspaces/{ws}/ai/*` with the same shapes as SovereignBaaS, so the existing AI Chat and AI Providers pages work on AeroBaaS; chat history in PostgreSQL; tests.
5. **If time remains:** Functions page in the admin (list, upload `.wasm`, run with a JSON payload, execution history).

Owner, in parallel (SovereignBaaS): Hetzner installer test; send IBAN, emails and domain for the website.

Moving portfolio projects onto SovereignBaaS: see `cargounetcom/sovereign-admin` → `docs/MIGRATION-PLAN.md` (step 0 on Hetzner first, then Nordic AI as the pilot).
