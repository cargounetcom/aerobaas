# AeroBaaS — agent-native backend

A backend for AI agents: workspaces and roles, scoped API keys, sandboxed functions, a local/cloud LLM and an MCP endpoint, in one Rust binary. Proprietary — © Aurora Ember Cyber Bio Lab (see `LICENSE`).

The admin UI is the **SovereignBaaS Admin** (engine adapter in Phase 3). Plan and status: **[ROADMAP.md](ROADMAP.md)**.

## What is real today (Phase 0)

| Area | Status |
|---|---|
| Sign-up, login, JWT, refresh, `/me` | ✅ works (secret from `AERO_JWT_SECRET`) |
| Workspaces, switch workspace, invites, members | ✅ works — ⚠️ **no membership checks yet** (Phase 1) |
| Scoped API keys, verify, credits | ✅ works — ⚠️ **routes not protected yet** (Phase 1) |
| Dependency scan (package.json, Cargo.toml, go.mod, pyproject) | ✅ works against a small built-in CVE list |
| Storage | 🟡 in memory — **data is lost on restart** (PostgreSQL in Phase 1) |
| Password hashing | 🟡 SHA-256 + fixed salt (Argon2id in Phase 1) |
| Request inspector, audit log | 🟡 endpoints exist, not recording yet (Phase 1) |
| Wasm functions | 🟡 stub: deploy stores bytes, execute returns a placeholder (Wasmtime in Phase 2) |
| AI generate / chat | 🟡 stub: fixed placeholder text (real models in Phase 2) |
| MCP | 🟡 WebSocket accepted, protocol not implemented (Phase 2) |

**Do not expose this server to the internet before Phase 1.**

## Run

```bash
cp .env.example .env            # set AERO_JWT_SECRET (openssl rand -base64 48)
set -a; . ./.env; set +a
cargo run --release --bin aero -- serve      # http://localhost:8080/health
```

Docker: `ghcr.io/cargounetcom/aerobaas:latest` (built by CI on every push to `main`).

```bash
docker run --rm -p 8080:8080 -e AERO_JWT_SECRET="$(openssl rand -base64 48)" ghcr.io/cargounetcom/aerobaas:latest
```

## Develop

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                   # must be green
cargo test -p aero-core --test api -- --ignored          # roadmap gaps (expected to fail until their phase)
```

## Layout

```
crates/
  aero-core          Axum server and routes (src/router.rs), integration tests (tests/api.rs)
  aero-cli           `aero` binary: serve, create-key, init, scan, deploy
  aero-auth          JWT, register/login, workspace switch
  aero-tenancy       workspaces, members, roles, invites
  aero-keys          scoped API keys and credits
  aero-db            storage (in-memory now; PostgreSQL in Phase 1)
  aero-wasm-runtime  function sandbox (stub until Phase 2)
  aero-ai-engine     AI inference + content validation (stub until Phase 2)
  aero-mcp-server    MCP tools (Phase 2)
  aero-remediation   dependency CVE scan
  aero-chat, aero-realtime, aero-inspector, aero-metrics, aero-skills, aero-crypto, aero-config, aero-protocol
openapi/aerobaas.yaml   API description
legacy/                 frontend prototype and Erlang orchestrator (retired; see ROADMAP)
```
