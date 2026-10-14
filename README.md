# AeroBaaS — Agent-Native Backend-as-a-Service

A next-generation serverless backend platform optimized for autonomous AI agents and code isolation.

## Architecture

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Core | Rust (Axum) | HTTP routing, middleware, single binary |
| Orchestrator | Erlang/OTP | Actor-model queue, agent coordination, rate limiting |
| Serverless Runtime | WebAssembly (Wasmtime) | <50ms cold start, sandboxed execution |
| AI Engine | Candle (Rust) | Embedded SLM (Qwen-2.5-Coder-1.5B), zero-latency inference |
| Database | SurrealDB | Graph/document hybrid, high-throughput |
| Security | Hazmat Protocol | Egress filtering, remediation pipeline, compliance logging |
| Auth | JWT + RBAC | Multi-tenancy, workspace switching, role-based access |
| Realtime | WebSocket | Audit event streaming, AI chat, live metrics |
| Frontend | React + shadcn/ui | Admin dashboard, AI chat, comfort panels |

## Project Structure

```
aerobaas/
├── crates/
│   ├── aero-core/           # Axum server, routing, middleware
│   ├── aero-wasm-runtime/   # Wasmtime sandbox + Hazmat egress
│   ├── aero-keys/           # Keys API 2.0, scopes, rate limiting
│   ├── aero-ai-engine/      # Candle SLM integration
│   ├── aero-mcp-server/     # MCP protocol server
│   ├── aero-db/             # SurrealDB integration
│   ├── aero-remediation/    # CVE scan + auto-patch pipeline
│   ├── aero-skills/         # Skill pack architecture
│   ├── aero-cli/            # `aero deploy --auto-detect`
│   ├── aero-config/         # Configuration management
│   ├── aero-crypto/         # AES-256-GCM encryption
│   ├── aero-protocol/       # Shared types & protocol definitions
│   ├── aero-auth/           # JWT auth, registration, login, workspace switch
│   ├── aero-tenancy/        # Multi-tenancy, workspaces, RBAC, invites
│   ├── aero-inspector/      # Request tracing & observability
│   ├── aero-chat/           # AI chat with comfort panels
│   ├── aero-realtime/       # Audit event logger + WebSocket broadcaster
│   └── aero-metrics/        # Wasm sandbox metrics collector
├── erlang/
│   └── aero_orchestrator/   # Erlang/OTP orchestrator app
├── frontend/
│   └── src/
│       ├── components/      # AI Chat, panels
│       ├── routes/admin/    # Audit log, metrics dashboard
│       ├── hooks/           # WebSocket, audit, metrics hooks
│       └── lib/             # API client
├── openapi/                 # OpenAPI 3.1 spec
├── benches/                 # Benchmark scripts
├── .github/workflows/       # CI
├── Dockerfile               # Dev/testing image
└── .env.example             # Environment config template
```

## Key Features

### Multi-Tenancy & RBAC
- **Workspace isolation** — each workspace has its own functions, keys, and members
- **Roles**: Owner → Admin → Manager → User (hierarchical permissions)
- **Invite flow** — email-based invitations with token acceptance
- **Workspace Switcher** — JWT token re-issued with workspace context

### Keys API 2.0
- Scoped API keys (`functions.read`, `executions.write`, etc.)
- Per-key function allowlists — `functions.list()` returns only permitted IDs
- Real-time credit tracking with auto-deactivation at zero balance
- AES-256-GCM encrypted storage

### Wasm Serverless Runtime
- Cold start < 50ms (Wasmtime, no Docker containers)
- Memory/CPU throttling per sandbox
- Hazmat egress filtering — blocks unauthorized network requests
- Shadow environment quarantine for new code

### Embedded AI Engine
- Local SLM (Qwen-2.5-Coder-1.5B) via Candle/Mistral.rs
- Zero-latency inference — no network roundtrips
- `Aero.ai.generateText()` from inside Wasm functions
- Content validation (prompt injection, spam, toxicity detection)
- Anomaly detection (DDoS, infinite loops)

### AI Chat with Comfort Panels
- Floating AI Copilot with MCP command execution
- Comfort panel with mood, greeting, quick actions
- Multiple panel layouts (Single, Split, Triple, Dashboard)
- 6 themes (Light, Dark, Midnight, Aurora, Sunset, Ocean)
- Token usage tracking, latency display

### Realtime Audit Log
- WebSocket event streaming for all system actions
- Actor types: User, AI Agent, System, Hazmat Module, Remediation Module
- Severity levels: Info, Warning, Critical, Security
- Filterable by actor type, action, severity, time range

### Wasm Sandbox Metrics
- Cold start latency history (P50, P95, P99)
- Memory throttling visualization
- Per-sandbox status (idle, running, quarantine, blocked)
- Egress blocking counter
- RPS tracking

### Remediation Pipeline
- CVE scanning for package.json, Cargo.toml, go.mod, pyproject.toml
- Auto-patching of vulnerable dependencies
- Compliance log generation with HMAC signatures
- Blocks deployment of High/Critical severity code

### MCP Server
- AI agents can read DB schema, create collections, generate & deploy functions
- Erlang lock manager prevents concurrent modifications by multiple agents
- 8 built-in tools (read_db_schema, create_collection, generate_function, etc.)

### Skills Architecture
- Framework auto-detection (Next.js, React, Rust/Axum, Python/FastAPI, etc.)
- Skill packs contain MCP prompt context + Wasm build tool + deployment targets
- `aero deploy --auto-detect` — single command deployment

## Quick Start

```bash
# Build the single binary
cargo build --release

# Run the server (loads embedded AI model on startup)
./target/release/aero serve

# Deploy with auto-detection
./target/release/aero deploy --auto-detect

# Create a scoped API key
./target/release/aero create-key --name "my-key" --scopes "functions.read,functions.list" --credits 100000

# Initialize a new project
./target/release/aero init .

# Scan dependencies
./target/release/aero scan .
```

## Frontend

```bash
cd frontend
npm install
npm run dev
```

The frontend provides:
- AI Chat Copilot (floating, available on all pages)
- Audit Log page (`/admin/logs`)
- Metrics Dashboard (`/admin/metrics`)
- Comfort Panel

## Deployment Modes

| Mode | Description |
|------|-------------|
| **AeroBaaS Cloud** | Public SaaS for indie developers and startups |
| **AeroBaaS Enterprise** | Private cloud (AWS/Azure) for enterprise clients |
| **AeroBaaS Local** | Air-gapped binary for gov/medical/banking |

## Benchmark

```bash
# Start the server
./target/release/aero serve

# Run benchmarks
python benches/benchmark.py
```

Acceptance criteria:
- ✅ Cold start < 50ms (P99)
- ✅ 50,000+ RPS on 4 vCPU / 8GB RAM
- ✅ Scoped key returns filtered function list
- ✅ MCP interface accepts external AI agent commands
- ✅ Embedded AI model loads on startup
- ✅ Wasm function can call local AI inference
- ✅ Core memory + model < 2GB idle

## License

AGPL-3.0
