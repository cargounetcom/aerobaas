# AeroBaaS — instructions for AI assistants working in this repo

## What this is
AeroBaaS is an agent-native backend: workspaces and roles, scoped API keys with credits, sandboxed Wasm functions, an AI layer and an MCP endpoint, in one Rust binary. It is closed-source commercial software of Aurora Ember Cyber Bio Lab (see `LICENSE`).

## Read first
* `ROADMAP.md`: phases, gates and what is done.
* `docs/IDEAS.md`: every idea from the original specs with keep/change/drop and its phase. **The decisions table there overrides `docs/source/`.**
* `README.md`: what is real today and what is a stub.

## Fixed decisions (don't change without the owner)
* Rust only. No Erlang, no Node/Python in the core.
* PostgreSQL through sqlx with migrations. No SurrealDB.
* The admin UI is the **SovereignBaaS Admin** (repo `cargounetcom/sovereign-admin`) with an `engine: aero` adapter. Don't revive `legacy/frontend`.
* AeroBaaS must also serve the SovereignBaaS admin API contract: `/api/me`, `/api/workspaces/{ws}/me|members|invites|inspector/*|ai/*`.
* No cryptocurrency customers or features.
* Secrets come only from the environment (`AERO_*`). The server refuses to start without `AERO_JWT_SECRET` (32+ characters).

## Rules for every change
1. `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` must pass.
2. Behaviour changes need a test in `crates/aero-core/tests/api.rs`. When a roadmap item lands, remove its `#[ignore = "Phase …"]`.
3. Every route checks authentication and workspace membership/role unless it is explicitly public (`/health`, register, login, key verify).
4. Never claim a feature works in docs or UI copy until a test proves it. Mark stubs as stubs.
5. Keep `README.md` "What is real today" and `ROADMAP.md` checkboxes in sync with the code.

## Useful commands
```bash
cargo run --release --bin aero -- serve                     # needs AERO_JWT_SECRET
cargo test -p aero-core --test api                           # API suite
cargo test -p aero-core --test api -- --ignored              # open roadmap gaps
```
