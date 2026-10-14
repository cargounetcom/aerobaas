# AeroBaaS Erlang/OTP Orchestrator

Actor-model based orchestration layer for managing millions of concurrent AI agent sessions.

## Modules

| Module | Purpose |
|--------|---------|
| `aero_orchestrator_sup` | Top-level supervisor tree |
| `aero_agent_sup` | Dynamic supervisor for AI agent processes |
| `aero_agent` | Individual agent gen_server (task execution) |
| `aero_rate_limiter` | ETS-based rate limiting with per-key tracking |
| `aero_lock_manager` | Distributed lock manager (prevents concurrent DB/function modifications) |
| `aero_credit_manager` | Real-time credit balance tracking and auto-deactivation |
| `aero_orchestrator_api` | Public API surface for Rust core integration |

## Build

```bash
rebar3 compile
rebar3 shell
```

## Integration with Rust Core

The Erlang orchestrator communicates with the Rust core via:
1. **Distributed Erlang** — Rust connects as a hidden node using `erlang-nif` or `rustler`
2. **HTTP bridge** — Simple HTTP API for cross-language calls
3. **Unix domain sockets** — Low-latency IPC for high-throughput scenarios

## Lock Manager Usage

Prevents two AI agents from simultaneously modifying the same resource:

```erlang
case aero_lock_manager:acquire(<<"table:users">>, AgentId) of
    {ok, acquired} ->
        %% Safe to modify
        modify_table(),
        aero_lock_manager:release(<<"table:users">>, AgentId);
    {error, {locked, Holder}} ->
        %% Wait or retry
        {error, resource_busy}
end.
```
