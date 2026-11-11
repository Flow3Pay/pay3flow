-- B2B crypto routing API v1 (docs/b2b-payment-api.md).
--
-- Money is stored as exact decimal strings or minor units. Nothing in this
-- schema is a floating-point column: the router must never lose precision
-- between a provider quote and the amount quoted to the business.

CREATE TABLE IF NOT EXISTS b2b_merchants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft','active','suspended')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS b2b_merchants_user_idx ON b2b_merchants (user_id);

-- Phase 1.1 API keys live in b2b_api_keys.sql (projects, keys, usage). V1 still
-- authenticates with the user Bearer token until the key middleware ships.

CREATE TABLE IF NOT EXISTS b2b_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    env TEXT NOT NULL CHECK (env IN ('test','live')),
    kind TEXT NOT NULL CHECK (kind IN ('transfer','checkout')),
    external_id TEXT,
    input_asset TEXT NOT NULL,
    input_network TEXT NOT NULL,
    output_asset TEXT NOT NULL,
    output_network TEXT NOT NULL,
    recipient TEXT NOT NULL,
    refund_address TEXT,
    status TEXT NOT NULL DEFAULT 'quoted',
    metadata JSONB NOT NULL DEFAULT '{}',
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, env, idempotency_key)
);

CREATE INDEX IF NOT EXISTS b2b_orders_merchant_created_idx
    ON b2b_orders (merchant_id, created_at DESC);
CREATE INDEX IF NOT EXISTS b2b_orders_status_idx ON b2b_orders (status);

CREATE TABLE IF NOT EXISTS b2b_quotes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    provider TEXT NOT NULL,
    provider_quote_id TEXT NOT NULL,
    exact TEXT NOT NULL CHECK (exact IN ('input','output')),
    input_amount TEXT NOT NULL,
    output_amount TEXT NOT NULL,
    best_output TEXT NOT NULL,
    captured_bps INTEGER NOT NULL DEFAULT 0,
    app_fee_bps INTEGER NOT NULL DEFAULT 0,
    deposit_address TEXT NOT NULL,
    refund_address TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    raw JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (order_id, provider_quote_id)
);

CREATE INDEX IF NOT EXISTS b2b_quotes_order_idx ON b2b_quotes (order_id, created_at DESC);

CREATE TABLE IF NOT EXISTS b2b_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    quote_id UUID REFERENCES b2b_quotes(id),
    kind TEXT NOT NULL CHECK (kind IN ('deposit','delivery','refund')),
    chain TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    from_address TEXT,
    amount TEXT NOT NULL,
    confirmations INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (chain, tx_hash, kind)
);

CREATE INDEX IF NOT EXISTS b2b_attempts_order_idx ON b2b_attempts (order_id);

CREATE TABLE IF NOT EXISTS b2b_events (
    id BIGSERIAL PRIMARY KEY,
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    kind TEXT NOT NULL,
    body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS b2b_events_order_idx ON b2b_events (order_id, id);

CREATE TABLE IF NOT EXISTS b2b_webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    url TEXT NOT NULL,
    secret_ciphertext BYTEA NOT NULL,
    events TEXT[] NOT NULL,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS b2b_webhook_endpoints_merchant_idx
    ON b2b_webhook_endpoints (merchant_id, active);

CREATE TABLE IF NOT EXISTS b2b_webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_id UUID NOT NULL REFERENCES b2b_webhook_endpoints(id),
    event_id BIGINT NOT NULL REFERENCES b2b_events(id),
    event_kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL DEFAULT 'pending',
    response_code INTEGER,
    response_body TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS b2b_webhook_deliveries_pending_idx
    ON b2b_webhook_deliveries (status, next_attempt_at);
CREATE INDEX IF NOT EXISTS b2b_webhook_deliveries_event_idx
    ON b2b_webhook_deliveries (event_id);

CREATE TABLE IF NOT EXISTS b2b_fee_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    order_id UUID REFERENCES b2b_orders(id),
    asset TEXT NOT NULL,
    amount TEXT NOT NULL,
    captured_bps INTEGER NOT NULL,
    recipient TEXT NOT NULL,
    tx_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS b2b_fee_ledger_merchant_idx
    ON b2b_fee_ledger (merchant_id, created_at DESC);

-- The accepted quote is mutable only through the quote/transfer transition; it
-- records which deposit address the business was told to fund after a re-quote.
ALTER TABLE b2b_orders
    ADD COLUMN IF NOT EXISTS accepted_quote_id UUID REFERENCES b2b_quotes(id);

-- Transfer creation has its own idempotency key: a quote may be requested with
-- one key and accepted later with another. NULLs do not collide in PostgreSQL.
ALTER TABLE b2b_orders ADD COLUMN IF NOT EXISTS transfer_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS b2b_orders_transfer_key_idx
    ON b2b_orders (merchant_id, env, transfer_key)
    WHERE transfer_key IS NOT NULL;

-- Hosted checkout pages. The token is the bearer secret in /pay/:token; the
-- order itself stays the source of truth for state.
CREATE TABLE IF NOT EXISTS b2b_checkout_sessions (
    token TEXT PRIMARY KEY,
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    success_url TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS b2b_checkout_sessions_order_idx
    ON b2b_checkout_sessions (order_id);

-- Fair polling rotates unchanged/temporarily unavailable orders without
-- changing their business updated_at timestamp.
ALTER TABLE b2b_orders ADD COLUMN IF NOT EXISTS last_polled_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS b2b_orders_poll_idx
    ON b2b_orders (last_polled_at NULLS FIRST, created_at, id)
    WHERE status IN ('awaiting_deposit','processing');
