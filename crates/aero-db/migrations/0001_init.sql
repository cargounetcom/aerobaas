-- AeroBaaS schema v1 (Phase 1). Applied automatically at startup.

CREATE TABLE users (
    id             UUID PRIMARY KEY,
    email          TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,
    password_hash  TEXT NOT NULL,
    avatar_url     TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    status         TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'deleted')),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE workspaces (
    id         UUID PRIMARY KEY,
    owner_id   UUID NOT NULL REFERENCES users(id),
    name       TEXT NOT NULL,
    slug       TEXT NOT NULL UNIQUE,
    plan       TEXT NOT NULL DEFAULT 'free',
    status     TEXT NOT NULL DEFAULT 'active',
    settings   JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE memberships (
    id           UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role         TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'manager', 'user')),
    invited_by   UUID,
    joined_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, user_id)
);
CREATE INDEX memberships_user ON memberships (user_id);

CREATE TABLE invites (
    id           UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    email        TEXT NOT NULL,
    role         TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'manager', 'user')),
    token_hash   TEXT NOT NULL UNIQUE,
    invited_by   UUID NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    accepted_at  TIMESTAMPTZ,
    accepted_by  UUID,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX invites_workspace ON invites (workspace_id);

CREATE TABLE api_keys (
    id                   UUID PRIMARY KEY,
    workspace_id         UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    created_by           UUID,
    name                 TEXT NOT NULL,
    key_prefix           TEXT NOT NULL,
    key_hash             TEXT NOT NULL UNIQUE,
    scopes               TEXT[] NOT NULL,
    allowed_function_ids UUID[] NOT NULL DEFAULT '{}',
    allowed_database_ids UUID[] NOT NULL DEFAULT '{}',
    credits              BIGINT NOT NULL,
    credits_limit        BIGINT NOT NULL,
    enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at           TIMESTAMPTZ,
    last_used_at         TIMESTAMPTZ
);
CREATE INDEX api_keys_workspace ON api_keys (workspace_id);

CREATE TABLE functions (
    id                UUID PRIMARY KEY,
    workspace_id      UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name              TEXT NOT NULL,
    runtime           TEXT NOT NULL,
    wasm_hash         TEXT NOT NULL,
    wasm              BYTEA NOT NULL,
    entrypoint        TEXT NOT NULL,
    memory_limit_mb   INTEGER NOT NULL,
    cpu_limit_percent INTEGER NOT NULL,
    timeout_ms        INTEGER NOT NULL,
    status            TEXT NOT NULL,
    created_by        UUID,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX functions_workspace ON functions (workspace_id);

-- Request inspector: one row per HTTP request (no bodies, invite tokens removed from paths).
CREATE TABLE requests (
    id           BIGSERIAL PRIMARY KEY,
    ts           TIMESTAMPTZ NOT NULL DEFAULT now(),
    method       TEXT NOT NULL,
    path         TEXT NOT NULL,
    status       INTEGER NOT NULL,
    latency_ms   INTEGER NOT NULL,
    user_id      UUID,
    key_id       UUID,
    workspace_id UUID,
    ip           TEXT,
    user_agent   TEXT,
    detail       TEXT
);
CREATE INDEX requests_workspace ON requests (workspace_id, id DESC);
CREATE INDEX requests_ts ON requests (ts);

-- Audit log: who did what (people, API keys/agents, system modules).
CREATE TABLE audit_events (
    id            UUID PRIMARY KEY,
    workspace_id  UUID,
    actor_type    TEXT NOT NULL,
    actor_id      UUID,
    actor_name    TEXT NOT NULL,
    action        TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id   TEXT,
    details       JSONB NOT NULL DEFAULT '{}',
    severity      TEXT NOT NULL,
    ip            TEXT,
    user_agent    TEXT,
    ts            TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_workspace ON audit_events (workspace_id, ts DESC);
