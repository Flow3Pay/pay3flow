CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Public service adoption and reputation. Counters are materialized so route
-- search responses can include them without aggregating the event tables.
CREATE TABLE IF NOT EXISTS services (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    executions_total BIGINT NOT NULL DEFAULT 0 CHECK (executions_total >= 0),
    likes_total BIGINT NOT NULL DEFAULT 0 CHECK (likes_total >= 0),
    dislikes_total BIGINT NOT NULL DEFAULT 0 CHECK (dislikes_total >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO services (slug, display_name)
VALUES
    ('binance', 'Binance'),
    ('bybit', 'Bybit'),
    ('okx', 'OKX'),
    ('bitget', 'Bitget'),
    ('rapira', 'Rapira')
ON CONFLICT (slug) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    updated_at = now()
WHERE services.display_name IS DISTINCT FROM EXCLUDED.display_name;

CREATE TABLE IF NOT EXISTS route_searches (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'searching'
        CHECK (status IN ('searching', 'finished', 'failed')),
    routes_found BIGINT NOT NULL DEFAULT 0 CHECK (routes_found >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS route_searches_created_idx
    ON route_searches (created_at DESC);

CREATE TABLE IF NOT EXISTS service_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    -- Keep execution evidence immutable: deleting a search must not silently
    -- remove executions while leaving its materialized service counter intact.
    search_id UUID NOT NULL REFERENCES route_searches(id),
    route_id TEXT NOT NULL,
    anonymous_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'started'
        CHECK (status IN ('started', 'success', 'failed', 'cancelled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (anonymous_id, service_id, search_id, route_id)
);

CREATE INDEX IF NOT EXISTS service_executions_service_idx
    ON service_executions (service_id, created_at DESC);
CREATE INDEX IF NOT EXISTS service_executions_anonymous_idx
    ON service_executions (anonymous_id, service_id);

CREATE TABLE IF NOT EXISTS service_votes (
    anonymous_id UUID NOT NULL,
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    vote TEXT NOT NULL CHECK (vote IN ('like', 'dislike')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (anonymous_id, service_id)
);

CREATE INDEX IF NOT EXISTS service_votes_service_idx
    ON service_votes (service_id);

-- Route feedback belongs to the concrete route shown to the viewer, not to
-- the exchange service used by one of its legs.
CREATE TABLE IF NOT EXISTS route_votes (
    anonymous_id UUID NOT NULL,
    route_id TEXT NOT NULL,
    vote TEXT NOT NULL CHECK (vote IN ('like', 'dislike')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (anonymous_id, route_id)
);

CREATE INDEX IF NOT EXISTS route_votes_route_idx
    ON route_votes (route_id);

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

-- Providerfile catalog. Providerfiles are compiled to idempotent SQL before
-- the Rust crate is rebuilt; the running application only reads these rows.
CREATE TABLE IF NOT EXISTS providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('buy', 'sell')),
    source_url TEXT NOT NULL,
    name TEXT NOT NULL,
    currencies TEXT[] NOT NULL DEFAULT '{}',
    banks TEXT[] NOT NULL DEFAULT '{}',
    adapter JSONB NOT NULL DEFAULT '{}'::JSONB,
    workflow JSONB NOT NULL DEFAULT '{}'::JSONB,
    fee_model JSONB NOT NULL DEFAULT '{}'::JSONB,
    status TEXT NOT NULL DEFAULT 'enabled'
        CHECK (status IN ('enabled', 'disabled')),
    source_file TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (slug, operation)
);

ALTER TABLE providers
    ADD COLUMN IF NOT EXISTS adapter JSONB NOT NULL DEFAULT '{}'::JSONB;
ALTER TABLE providers
    ADD COLUMN IF NOT EXISTS workflow JSONB NOT NULL DEFAULT '{}'::JSONB;
ALTER TABLE providers
    ADD COLUMN IF NOT EXISTS fee_model JSONB NOT NULL DEFAULT '{}'::JSONB;

CREATE INDEX IF NOT EXISTS providers_status_operation_idx
    ON providers (status, operation);
CREATE INDEX IF NOT EXISTS providers_currencies_idx
    ON providers USING GIN (currencies);
CREATE INDEX IF NOT EXISTS providers_banks_idx
    ON providers USING GIN (banks);

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

-- Crypto networks exposed by GET /api/networks and used to validate route
-- requests. The rows themselves live in the catalog data migration.
CREATE TABLE IF NOT EXISTS crypto_networks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    currencies TEXT[] NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'enabled'
        CHECK (status IN ('enabled', 'disabled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS crypto_networks_status_idx
    ON crypto_networks (status, slug);
CREATE INDEX IF NOT EXISTS crypto_networks_currencies_idx
    ON crypto_networks USING GIN (currencies);

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
    (source_country, source_currency, target_country, target_currency, status, min_amount_minor, max_amount_minor, daily_limit_minor, metadata)
VALUES
    ('AM', 'AMD', 'RU', 'RUB', 'enabled', 1000, NULL, 5000000000, '{"mvp": true, "label": "Armenia AMD to Russia RUB"}')
ON CONFLICT (source_country, source_currency, target_country, target_currency)
DO UPDATE SET
    min_amount_minor = EXCLUDED.min_amount_minor,
    daily_limit_minor = COALESCE(exchange_corridors.daily_limit_minor, EXCLUDED.daily_limit_minor),
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
    correlation_id UUID NOT NULL DEFAULT gen_random_uuid(),
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

ALTER TABLE exchange_orders ADD COLUMN IF NOT EXISTS correlation_id UUID NOT NULL DEFAULT gen_random_uuid();
CREATE INDEX IF NOT EXISTS exchange_orders_correlation_idx
    ON exchange_orders (correlation_id);

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

-- Development live-search profiles. Even mock candidates are data returned to
-- clients, so their catalog is migrated rather than compiled into Rust.
CREATE TABLE IF NOT EXISTS mock_route_profiles (
    slug TEXT PRIMARY KEY,
    asset TEXT NOT NULL,
    network TEXT NOT NULL,
    entry_provider TEXT NOT NULL,
    exit_provider TEXT NOT NULL,
    entry_delay_ms BIGINT NOT NULL CHECK (entry_delay_ms >= 0),
    exit_delay_ms BIGINT NOT NULL CHECK (exit_delay_ms >= 0),
    rate_bps BIGINT NOT NULL,
    fee_minor BIGINT NOT NULL CHECK (fee_minor >= 0),
    eta_minutes INTEGER NOT NULL CHECK (eta_minutes > 0),
    spread_bps INTEGER NOT NULL,
    risk_score INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'enabled'
        CHECK (status IN ('enabled', 'disabled')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

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

CREATE TABLE IF NOT EXISTS token_ledger_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_type TEXT NOT NULL,
    owner_id UUID NOT NULL,
    currency TEXT NOT NULL,
    available_minor BIGINT NOT NULL DEFAULT 0,
    reserved_minor BIGINT NOT NULL DEFAULT 0,
    locked_minor BIGINT NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (owner_type, owner_id, currency),
    CHECK (available_minor >= 0),
    CHECK (reserved_minor >= 0),
    CHECK (locked_minor >= 0)
);

CREATE INDEX IF NOT EXISTS token_ledger_accounts_owner_idx
    ON token_ledger_accounts (owner_type, owner_id);

CREATE TABLE IF NOT EXISTS token_ledger_operations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    idempotency_key TEXT NOT NULL UNIQUE,
    account_id UUID NOT NULL REFERENCES token_ledger_accounts(id) ON DELETE CASCADE,
    operation_type TEXT NOT NULL,
    amount_minor BIGINT NOT NULL,
    currency TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'applied',
    related_order_id UUID REFERENCES exchange_orders(id) ON DELETE SET NULL,
    related_settlement_id UUID,
    previous_operation_id UUID REFERENCES token_ledger_operations(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS token_ledger_operations_account_idx
    ON token_ledger_operations (account_id, created_at);
CREATE INDEX IF NOT EXISTS token_ledger_operations_order_idx
    ON token_ledger_operations (related_order_id, created_at);

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

-- Runtime safety gates. The singleton row is intentionally database-backed so
-- operations can stop the exchange without rebuilding or redeploying it.
CREATE TABLE IF NOT EXISTS exchange_controls (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    exchange_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    user_daily_limit_minor BIGINT NOT NULL DEFAULT 1000000000 CHECK (user_daily_limit_minor > 0),
    solver_daily_limit_minor BIGINT NOT NULL DEFAULT 5000000000 CHECK (solver_daily_limit_minor > 0),
    manual_review_threshold_minor BIGINT CHECK (manual_review_threshold_minor > 0),
    terms_version TEXT NOT NULL DEFAULT '2026-09-19',
    updated_by TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO exchange_controls
    (singleton, exchange_enabled, user_daily_limit_minor, solver_daily_limit_minor,
     manual_review_threshold_minor, terms_version)
VALUES (TRUE, TRUE, 1000000000, 5000000000, 50000000, '2026-09-19')
ON CONFLICT (singleton) DO NOTHING;

CREATE TABLE IF NOT EXISTS exchange_manual_reviews (
    order_id UUID PRIMARY KEY REFERENCES exchange_orders(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending',
    reason TEXT NOT NULL,
    resolved_by TEXT,
    resolution_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS exchange_manual_reviews_status_idx
    ON exchange_manual_reviews (status, created_at);

CREATE TABLE IF NOT EXISTS exchange_consents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES exchange_orders(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    terms_version TEXT NOT NULL,
    settlement_asset_disclosure BOOLEAN NOT NULL,
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (order_id, terms_version)
);

CREATE INDEX IF NOT EXISTS exchange_consents_order_idx
    ON exchange_consents (order_id, accepted_at);
