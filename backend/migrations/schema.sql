CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS activitypub_deliveries (
    activity_id TEXT NOT NULL,
    target TEXT NOT NULL,
    delivered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (activity_id, target)
);

CREATE TABLE IF NOT EXISTS activitypub_inbox (
    activity_id TEXT PRIMARY KEY,
    source TEXT NOT NULL DEFAULT '',
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- === Phase 2: payments domain ===

-- Acquirer passport: geo, currencies, fees, limits, endpoints, active flag.
CREATE TABLE IF NOT EXISTS acquirers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    geo TEXT NOT NULL DEFAULT '',
    currencies TEXT NOT NULL DEFAULT '',
    fee_percent DOUBLE PRECISION NOT NULL DEFAULT 0,
    fee_fixed BIGINT NOT NULL DEFAULT 0,
    min_amount BIGINT,
    max_amount BIGINT,
    amount_currency TEXT,
    endpoints JSONB NOT NULL DEFAULT '{}',
    website_url TEXT NOT NULL DEFAULT '',
    api_docs_url TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Self-heal acquirer tables created before the discovery columns existed
-- (CREATE TABLE IF NOT EXISTS does not alter an existing table).
ALTER TABLE acquirers ADD COLUMN IF NOT EXISTS website_url TEXT NOT NULL DEFAULT '';
ALTER TABLE acquirers ADD COLUMN IF NOT EXISTS api_docs_url TEXT NOT NULL DEFAULT '';
ALTER TABLE acquirers ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '';
ALTER TABLE acquirers ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT '';

-- Provider secrets, kept separate from regular acquisitions.
CREATE TABLE IF NOT EXISTS credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    acquirer_id UUID NOT NULL REFERENCES acquirers(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    value_encrypted TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (acquirer_id, name)
);

-- Incoming provider webhook events + processing status.
CREATE TABLE IF NOT EXISTS provider_webhooks (
    id BIGSERIAL PRIMARY KEY,
    provider TEXT NOT NULL,
    event_id TEXT,
    event_type TEXT,
    payload JSONB NOT NULL,
    signature TEXT,
    status TEXT NOT NULL DEFAULT 'received',
    error TEXT,
    transaction_id UUID,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ
);

-- Core transaction record.
CREATE TABLE IF NOT EXISTS transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',
    from_amount BIGINT NOT NULL,
    from_currency TEXT NOT NULL,
    to_amount BIGINT,
    to_currency TEXT,
    from_account TEXT NOT NULL DEFAULT '',
    to_account TEXT NOT NULL,
    method TEXT,
    fees BIGINT NOT NULL DEFAULT 0,
    provider TEXT,
    external_id TEXT,
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS transactions_idem_key_idx
    ON transactions (idempotency_key) WHERE idempotency_key IS NOT NULL;

-- Route: ties a transaction to the chosen acquirer + execution params.
CREATE TABLE IF NOT EXISTS routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    acquirer_id UUID REFERENCES acquirers(id),
    acquirer_slug TEXT NOT NULL,
    fee_percent DOUBLE PRECISION NOT NULL DEFAULT 0,
    exchange_rate DOUBLE PRECISION,
    status TEXT NOT NULL DEFAULT 'pending',
    source TEXT NOT NULL DEFAULT 'fallback',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS transactions_user_idx ON transactions (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS routes_transaction_idx ON routes (transaction_id);

-- === PLAN 2△ / 46a: bank exchange-pair router ===
-- "which card of which sender bank can pay which card of which recipient bank".
-- The catalog lives here (not hardcoded in the frontend) and is served by
-- GET /api/exchange-pairs; status/limits are managed via /api/admin/exchange-pairs.
CREATE TABLE IF NOT EXISTS exchange_pairs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_scheme TEXT NOT NULL,
    from_bank TEXT NOT NULL,
    from_bank_icon_url TEXT NOT NULL DEFAULT '',
    to_scheme TEXT NOT NULL,
    to_bank TEXT NOT NULL,
    to_bank_icon_url TEXT NOT NULL DEFAULT '',
    country TEXT NOT NULL DEFAULT '',
    currencies TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'enabled',
    daily_limit_minor BIGINT,
    daily_limit_currency TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS exchange_pairs_route_idx
    ON exchange_pairs (from_scheme, from_bank, to_scheme, to_bank);
CREATE INDEX IF NOT EXISTS exchange_pairs_status_idx
    ON exchange_pairs (status);

-- === PLAN 2△ / banks: the bank directory the payment form picks from ===
-- The user composes the exchange ("from bank → to bank") themselves, so the
-- backend owns the canonical, worldwide list of banks (>=200). Each bank's
-- icon comes from its own site favicon (domain), not a shipped asset.
CREATE TABLE IF NOT EXISTS banks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'both',      -- sender | receiver | both
    country TEXT NOT NULL DEFAULT '',
    currency TEXT NOT NULL DEFAULT '',
    domain TEXT NOT NULL DEFAULT '',
    icon_url TEXT NOT NULL DEFAULT '',
    schemes TEXT NOT NULL DEFAULT '',        -- comma-separated card schemes
    status TEXT NOT NULL DEFAULT 'enabled',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS banks_name_idx ON banks (name);
CREATE INDEX IF NOT EXISTS banks_status_idx ON banks (status);
CREATE INDEX IF NOT EXISTS banks_role_idx ON banks (role);

-- === PLAN2 EX-2: solver-based exchange domain ===

CREATE TABLE IF NOT EXISTS exchange_corridors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_country TEXT NOT NULL,
    source_currency TEXT NOT NULL,
    target_country TEXT NOT NULL,
    target_currency TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'enabled',
    min_amount_minor BIGINT,
    max_amount_minor BIGINT,
    daily_limit_minor BIGINT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (source_country, source_currency, target_country, target_currency)
);

CREATE INDEX IF NOT EXISTS exchange_corridors_status_idx
    ON exchange_corridors (status);

INSERT INTO exchange_corridors
    (source_country, source_currency, target_country, target_currency, status, min_amount_minor, max_amount_minor, metadata)
VALUES
    ('AM', 'AMD', 'RU', 'RUB', 'enabled', 1000, NULL, '{"mvp": true, "label": "Armenia AMD to Russia RUB"}')
ON CONFLICT (source_country, source_currency, target_country, target_currency)
DO UPDATE SET
    status = EXCLUDED.status,
    min_amount_minor = EXCLUDED.min_amount_minor,
    metadata = exchange_corridors.metadata || EXCLUDED.metadata,
    updated_at = now();

CREATE TABLE IF NOT EXISTS exchange_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    source_country TEXT NOT NULL,
    source_currency TEXT NOT NULL,
    source_amount_minor BIGINT NOT NULL,
    source_method_type TEXT NOT NULL,
    source_method_ref TEXT,
    target_country TEXT NOT NULL,
    target_currency TEXT NOT NULL,
    target_amount_min_minor BIGINT,
    target_method_type TEXT NOT NULL,
    target_method_ref TEXT,
    funding_instruction_id UUID,
    funding_status TEXT NOT NULL DEFAULT 'not_started',
    status TEXT NOT NULL DEFAULT 'created',
    deadline_at TIMESTAMPTZ,
    selected_quote_id UUID,
    failure_code TEXT,
    failure_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS exchange_orders_user_created_idx
    ON exchange_orders (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS exchange_orders_status_created_idx
    ON exchange_orders (status, created_at);

CREATE TABLE IF NOT EXISTS exchange_solvers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT UNIQUE NOT NULL,
    actor_id TEXT,
    handle TEXT,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'discovered',
    countries JSONB NOT NULL DEFAULT '[]',
    currencies JSONB NOT NULL DEFAULT '[]',
    rails JSONB NOT NULL DEFAULT '[]',
    min_amount_minor BIGINT,
    max_amount_minor BIGINT,
    fee_model JSONB NOT NULL DEFAULT '{}',
    risk_score INTEGER NOT NULL DEFAULT 50,
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS exchange_solvers_status_idx
    ON exchange_solvers (status);

CREATE TABLE IF NOT EXISTS exchange_quotes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES exchange_orders(id) ON DELETE CASCADE,
    solver_id UUID NOT NULL REFERENCES exchange_solvers(id) ON DELETE CASCADE,
    source_amount_minor BIGINT NOT NULL,
    target_amount_minor BIGINT NOT NULL,
    source_currency TEXT NOT NULL,
    target_currency TEXT NOT NULL,
    funding_method_type TEXT NOT NULL,
    requires_user_funding BOOLEAN NOT NULL DEFAULT TRUE,
    rate NUMERIC(24, 12) NOT NULL,
    fee_minor BIGINT NOT NULL,
    eta_minutes INTEGER NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'received',
    settlement_plan JSONB NOT NULL DEFAULT '{}',
    risk_score INTEGER NOT NULL,
    score BIGINT,
    raw_response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS exchange_quotes_order_idx
    ON exchange_quotes (order_id, created_at);
CREATE INDEX IF NOT EXISTS exchange_quotes_solver_idx
    ON exchange_quotes (solver_id, created_at);
CREATE INDEX IF NOT EXISTS exchange_quotes_status_expires_idx
    ON exchange_quotes (status, expires_at);

CREATE TABLE IF NOT EXISTS funding_instructions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES exchange_orders(id) ON DELETE CASCADE,
    quote_id UUID NOT NULL REFERENCES exchange_quotes(id) ON DELETE CASCADE,
    solver_id UUID NOT NULL REFERENCES exchange_solvers(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'created',
    method_type TEXT NOT NULL,
    amount_minor BIGINT NOT NULL,
    currency TEXT NOT NULL,
    destination_ref TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    user_confirmed_at TIMESTAMPTZ,
    raw_payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS funding_instructions_order_idx
    ON funding_instructions (order_id, created_at);
CREATE INDEX IF NOT EXISTS funding_instructions_status_idx
    ON funding_instructions (status, expires_at);

CREATE TABLE IF NOT EXISTS exchange_settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES exchange_orders(id) ON DELETE CASCADE,
    quote_id UUID NOT NULL REFERENCES exchange_quotes(id) ON DELETE CASCADE,
    solver_id UUID NOT NULL REFERENCES exchange_solvers(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    token_leg_status TEXT NOT NULL,
    money_leg_status TEXT NOT NULL,
    funding_status TEXT NOT NULL,
    pay3flow_wallet_ref TEXT,
    token_ledger_ref TEXT,
    money_reference TEXT,
    proof_id UUID,
    failure_code TEXT,
    failure_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS exchange_settlements_order_idx
    ON exchange_settlements (order_id);
CREATE INDEX IF NOT EXISTS exchange_settlements_status_idx
    ON exchange_settlements (status, created_at);

CREATE TABLE IF NOT EXISTS exchange_proofs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    settlement_id UUID NOT NULL REFERENCES exchange_settlements(id) ON DELETE CASCADE,
    solver_id UUID NOT NULL REFERENCES exchange_solvers(id) ON DELETE CASCADE,
    proof_type TEXT NOT NULL,
    proof_payload JSONB NOT NULL,
    verification_status TEXT NOT NULL DEFAULT 'pending',
    verified_by TEXT,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS exchange_proofs_settlement_idx
    ON exchange_proofs (settlement_id, created_at);

CREATE TABLE IF NOT EXISTS audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT,
    payload JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS audit_events_entity_idx
    ON audit_events (entity_type, entity_id, created_at);
CREATE INDEX IF NOT EXISTS audit_events_type_idx
    ON audit_events (event_type, created_at);
