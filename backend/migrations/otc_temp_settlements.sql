-- Settlement state for stablecoin temporary wallets (spec phases D/E).
-- One row per deal that uses a temporary USD wallet.
CREATE TABLE IF NOT EXISTS otc_temp_settlements (
 trade_id UUID PRIMARY KEY REFERENCES otc_p2p_trades(id) ON DELETE CASCADE,
 wallet_id UUID NOT NULL REFERENCES otc_temp_wallets(id),
 state TEXT NOT NULL DEFAULT 'awaiting_payment'
   CHECK(state IN ('awaiting_payment','payout_claim','payout_submitted','fee_submitted','fee_retained','settled','failed')),
 reference TEXT,
 fee_reference TEXT,
 attempts INTEGER NOT NULL DEFAULT 0,
 last_error TEXT,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS otc_temp_settlements_due
 ON otc_temp_settlements(state, updated_at);
