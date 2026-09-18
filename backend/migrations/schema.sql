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