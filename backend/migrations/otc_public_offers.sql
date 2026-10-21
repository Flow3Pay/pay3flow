-- Public advertisements are separate from accepted desk trades and reservations.
-- Posting an advertisement never creates a transfer or a payment instruction.
CREATE TABLE IF NOT EXISTS otc_public_offers (
 id UUID PRIMARY KEY,
 actor TEXT NOT NULL,
 seller_ever TEXT NOT NULL,
 body JSONB NOT NULL,
 state TEXT NOT NULL DEFAULT 'open' CHECK(state IN ('open','cancelled')),
 idempotency_key TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 expires_at TIMESTAMPTZ NOT NULL DEFAULT now() + interval '24 hours',
 UNIQUE(actor,idempotency_key)
);
CREATE INDEX IF NOT EXISTS otc_public_offers_recent ON otc_public_offers(created_at DESC);
