-- Peer-to-peer settlement of public offers. Two independent transfer legs,
-- verified from chain evidence; deliberately separate from desk trades.
CREATE TABLE IF NOT EXISTS otc_p2p_trades (
 id UUID PRIMARY KEY,
 offer_id UUID NOT NULL REFERENCES otc_public_offers(id),
 owner_actor TEXT NOT NULL,
 taker_actor TEXT NOT NULL,
 body JSONB NOT NULL,
 state TEXT NOT NULL DEFAULT 'payment_pending'
   CHECK(state IN ('payment_pending','delivery_pending','completed','expired','cancelled')),
 idempotency_key TEXT NOT NULL,
 pay_by TIMESTAMPTZ NOT NULL,
 payout_by TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(taker_actor,idempotency_key)
);
CREATE INDEX IF NOT EXISTS otc_p2p_trades_recent ON otc_p2p_trades(created_at DESC);
CREATE TABLE IF NOT EXISTS otc_p2p_attempts (
 id UUID PRIMARY KEY,
 trade_id UUID NOT NULL REFERENCES otc_p2p_trades(id),
 kind TEXT NOT NULL CHECK(kind IN ('payment','payout')),
 payer_actor TEXT NOT NULL,
 chain TEXT NOT NULL CHECK(chain IN ('ethereum','everscale')),
 state TEXT NOT NULL DEFAULT 'prepared'
   CHECK(state IN ('prepared','submitted','confirming','verified','failed')),
 instructions JSONB NOT NULL,
 reference TEXT,
 evidence JSONB,
 idempotency_key TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(trade_id,kind,idempotency_key)
);
-- Fresh databases create the narrow CHECK; widen it in place once 'taken' exists.
ALTER TABLE otc_public_offers DROP CONSTRAINT IF EXISTS otc_public_offers_state_check;
ALTER TABLE otc_public_offers
 ADD CONSTRAINT otc_public_offers_state_check
 CHECK(state IN ('open','cancelled','taken','settled'));
