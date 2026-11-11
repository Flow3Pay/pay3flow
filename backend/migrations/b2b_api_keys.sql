-- Phase 1.1 merchant API keys (docs/b2b-merchant-portal.md §5.3, §6, §10 step 1).
--
-- Projects are the folder for keys and the reporting unit for traffic and
-- revenue. Keys always belong to exactly one project. Applied after b2b.sql.

-- Projects are the folder for API keys and the unit for traffic and revenue
-- reporting.
CREATE TABLE IF NOT EXISTS b2b_projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','archived')),
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS b2b_projects_name_idx
    ON b2b_projects (merchant_id, lower(name));

CREATE INDEX IF NOT EXISTS b2b_projects_merchant_idx
    ON b2b_projects (merchant_id, created_at DESC);

-- Phase 1.1 lands project-scoped from the start: name, environment, scopes and
-- the daily request limit sit on the key; the secret is stored as a hash only.
CREATE TABLE IF NOT EXISTS b2b_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES b2b_projects(id),
    name TEXT NOT NULL,
    env TEXT NOT NULL CHECK (env IN ('test','live')),
    prefix TEXT NOT NULL UNIQUE,
    secret_hash TEXT NOT NULL,
    scopes TEXT[] NOT NULL CHECK (cardinality(scopes) > 0),
    daily_request_limit INT NOT NULL DEFAULT 0
        CHECK (daily_request_limit >= 0),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','revoked')),
    created_by UUID REFERENCES users(id),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS b2b_api_keys_name_idx
    ON b2b_api_keys (project_id, lower(name));

CREATE INDEX IF NOT EXISTS b2b_api_keys_project_idx
    ON b2b_api_keys (project_id, status, created_at DESC);

-- One row per key and hour, written by the API middleware. Day and month views
-- aggregate this table; the Activity page never scans raw request logs.
CREATE TABLE IF NOT EXISTS b2b_api_key_usage (
    key_id UUID NOT NULL REFERENCES b2b_api_keys(id) ON DELETE CASCADE,
    bucket_start TIMESTAMPTZ NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0,
    errors_4xx BIGINT NOT NULL DEFAULT 0,
    errors_5xx BIGINT NOT NULL DEFAULT 0,
    latency_sum_ms BIGINT NOT NULL DEFAULT 0,
    latency_p50_ms INT,
    latency_p95_ms INT,
    PRIMARY KEY (key_id, bucket_start)
);

-- Attribution for Activity and the per-project cashbox. Rows created before
-- this migration keep NULL and appear as "Без проекта".
ALTER TABLE b2b_orders
    ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES b2b_projects(id);
ALTER TABLE b2b_orders
    ADD COLUMN IF NOT EXISTS api_key_id UUID REFERENCES b2b_api_keys(id);
ALTER TABLE b2b_fee_ledger
    ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES b2b_projects(id);

CREATE INDEX IF NOT EXISTS b2b_orders_project_idx
    ON b2b_orders (merchant_id, project_id, created_at DESC);
CREATE INDEX IF NOT EXISTS b2b_fee_ledger_project_idx
    ON b2b_fee_ledger (merchant_id, project_id, created_at DESC);
