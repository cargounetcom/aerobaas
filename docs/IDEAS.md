# Idea library

This library collects every idea from the original specs in `docs/source/` and records whether we keep it, change it or drop it, and where it lands in [ROADMAP.md](../ROADMAP.md).

The originals stay unchanged as reference. When this file and the originals disagree, this file wins.

## Decisions that override the original spec

| Original spec | Decision (14 Oct 2026) | Why |
|---|---|---|
| Erlang/OTP orchestrator | **Rust** (tokio, PostgreSQL advisory locks, queues in Postgres) | One language for a small team. The locking and credit features keep working the same way. |
| SurrealDB / ScyllaDB | **PostgreSQL** (sqlx, migrations) | Proven, backups and auditing are easy, and regulated clients already know it. Graph-style links can use Postgres tables or JSONB. |
| Own React admin (`frontend/`) | **SovereignBaaS Admin** with `engine: aero` | Reuses a tested UI that already has workspaces, roles, invites, inspector and AI chat. |
| AGPL-3.0 | **Closed commercial licence** | Sold with licence keys (`AERO-…`). |
| "No Docker" | Ship a **single binary** and a Docker image | Clients choose. The binary stays the headline. |

## Feature ideas

| # | Idea (source) | Status | Phase / notes |
|---|---|---|---|
| A1 | Wasm runtime on Wasmtime, cold start < 50 ms (spec 2.A) | ✅ keep | Phase 2. Measure p50/p95 with a benchmark in CI. |
| A2 | Memory and CPU throttling per sandbox (2.A) | ✅ keep | Phase 2. Uses Wasmtime fuel and memory limits. |
| B1 | Keys API 2.0: scopes, per-key function allow-list, `functions.list()` filtered (2.B) | ✅ done | Phase 1 (14 Oct 2026). Tested in `key_scopes_allow_list_and_credits`. |
| B2 | Real-time credits, key turns off at 0 (2.B) | ✅ done | Phase 1: atomic in PostgreSQL, 1 credit per request. Linking credits to the unified licensing service is Phase 5. |
| C1 | Built-in MCP server: read schema, create collections, generate and deploy functions (2.C) | ✅ keep | Phase 2. JSON-RPC over WebSocket, 5 tools first. |
| C2 | Lock so two agents can't change the same table or function at once (2.C) | ✅ keep, in Rust | Phase 4. Postgres advisory locks instead of Erlang. |
| D1 | Hazmat "shadow environment" before deploy (2.D) | ✅ keep | Phase 2. First test run happens in a quarantined sandbox with no network. |
| D2 | Egress filtering: block unknown outbound IPs, log a security event (2.D) | ✅ keep | Phase 2. Host allow-list enforced by the runtime's network shim. |
| E1 | Embedded SLM (Qwen-2.5-Coder 1.5B via Candle) in the binary (AI add-on) | 🟡 later | Phase 2 starts with an OpenAI-compatible endpoint (Ollama, OpenRouter, Gemini, MiMo). Embedded Candle comes after as an "air-gapped" option. Memory target: ≤ 2 GB idle with the model. |
| E2 | `Aero.ai.generateText()` callable from inside Wasm (AI add-on) | ✅ keep | Phase 2. Host function exposed to sandboxes. |
| E3 | Automatic index creation from slow queries (AI add-on) | 🟡 later | After Phase 5. Starts as "suggest index" with human approval, never fully automatic for regulated clients. |
| E4 | Anomaly detection: DDoS, infinite loops; isolate the stream (AI add-on) | ✅ keep, rules first | Phase 2–4. Rate limits and time/fuel limits first, model-based detection later. |
| F1 | CVE scan of package.json, Cargo.toml, go.mod, pyproject (security add-on) | ✅ exists | Phase 2: switch the built-in list to OSV.dev data, mirrored for air-gapped installs. |
| F2 | Auto-patch to a safe version (security add-on) | 🟡 limited | Suggest the fixed version and open a change for approval. No silent code rewrites for regulated clients. |
| F3 | Block deploy of High/Critical findings (security add-on) | ✅ keep | Phase 2 gate. |
| F4 | Prompt-injection screening of function inputs (security add-on) | ✅ keep | Pattern check exists; model-based later. |
| F5 | Signed compliance log per deploy (security add-on) | ✅ keep, reworded | Phase 2: HMAC/ed25519-signed deploy record (who, what, scan result, hash). Maps to **EU** frameworks (GDPR, NIS2, DORA, CRA). Don't claim conformity to "Israeli Cybersecurity Ops" standards unless audited. |
| G1 | AI command chat with MCP actions (admin doc §1) | ✅ keep | Phase 3. The SovereignBaaS Admin chat gains "actions" that call MCP tools, each needing confirmation. |
| G2 | Realtime audit log: who/what (user, AI agent, system, Hazmat) (admin doc A) | 🟡 half done | Phase 1: recorded in PostgreSQL + live WebSocket `/v1/audit/stream`. Phase 3: the admin page. |
| G3 | Sandbox metrics dashboard: cold-start latency, memory throttling (admin doc B) | ✅ keep | Phase 3. Recharts page in the admin. |
| H1 | Delivery models: Cloud, Enterprise private cloud, Local air-gapped (business section) | ✅ keep | Local and Enterprise first (same model as SovereignBaaS). Cloud later. |
| H2 | Sector packaging: Gov, MedTech, FinTech (business section) | ✅ keep | Website and sales material. **No crypto clients.** HIPAA wording only if actually assessed; for the EU use GDPR, NIS2 and DORA. |
| H3 | Freemium cloud for developers (business section) | 🟡 later | After Phase 5. |

## Claims to correct before marketing

* **"50,000 RPS on a $5 server"** and **"10× faster than Appwrite"**: unproven. Publish only measured numbers from the Phase 4 load test, with the hardware stated. The working target is 5–10k req/s on 4 vCPU for authenticated API calls.
* **"< 50 ms cold start"**: realistic for small Wasm modules with Wasmtime and pre-compiled modules. It still needs a benchmark before it goes on the website.
* **"Single binary with embedded model"**: possible, but the model file adds about 1 GB. Offer it as a separate air-gapped package.

## How to use this file when writing prompts

1. Name the item IDs, e.g. "implement B1 + B2 from docs/IDEAS.md".
2. Every task must state its acceptance test in `crates/aero-core/tests/api.rs` (un-ignore the matching test or add a new one).
3. Keep the decisions table above. Don't reintroduce Erlang, SurrealDB or the old frontend.
