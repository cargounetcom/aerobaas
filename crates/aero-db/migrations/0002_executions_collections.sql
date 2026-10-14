-- Phase 2: function execution history, and workspace tables (collections of JSON rows).

CREATE TABLE executions (
    id           UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    function_id  UUID NOT NULL REFERENCES functions(id) ON DELETE CASCADE,
    user_id      UUID,
    key_id       UUID,
    status       TEXT NOT NULL,
    duration_ms  INTEGER NOT NULL,
    fuel_used    BIGINT NOT NULL,
    memory_mb    DOUBLE PRECISION NOT NULL,
    cold_start   BOOLEAN NOT NULL,
    error        TEXT,
    ts           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX executions_function ON executions (function_id, ts DESC);
CREATE INDEX executions_workspace ON executions (workspace_id, ts DESC);

CREATE TABLE collections (
    id           UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    columns      JSONB NOT NULL,
    created_by   UUID,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, name)
);

CREATE TABLE collection_rows (
    id            UUID PRIMARY KEY,
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    data          JSONB NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX collection_rows_by_time ON collection_rows (collection_id, created_at DESC);
CREATE INDEX collection_rows_data ON collection_rows USING GIN (data jsonb_path_ops);
