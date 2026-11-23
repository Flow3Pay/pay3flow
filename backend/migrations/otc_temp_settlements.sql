-- Settlement state for stablecoin temporary wallets (spec phases D/E).
-- One row per deal that uses a temporary USD wallet.
CREATE TABLE IF NOT EXISTS otc_temp_settlements (
 trade_id UUID PRIMARY KEY REFERENCES otc_p2p_trades(id) ON DELETE CASCADE,
 wallet_id UUID NOT NULL REFERENCES otc_temp_wallets(id),
 state TEXT NOT NULL DEFAULT 'awaiting_payment'
   CHECK(state IN ('awaiting_payment','payout_claim','payout_submitted','intents_submitted','intents_payout_claim','intents_payout_submitted','intents_refunded','fee_submitted','fee_retained','stalled','settled','failed')),
 reference TEXT,
 fee_reference TEXT,
 deposit_address TEXT,
 deposit_memo TEXT,
 quote_id TEXT,
 source_units TEXT,
 minimum_units TEXT,
 -- Net paid from the platform reserve instead of the temporary wallet; the
 -- withheld deposit is then swept in full to the fee recipient.
 reserve_units TEXT,
 output_signed_transaction BYTEA,
 payout_signed_transaction BYTEA,
 fee_signed_transaction BYTEA,
 attempts INTEGER NOT NULL DEFAULT 0,
 last_error TEXT,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Tables created by an earlier build gain the Intents columns in place.
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS deposit_address TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS deposit_memo TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS quote_id TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS source_units TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS minimum_units TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS reserve_units TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS output_signed_transaction BYTEA;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS payout_signed_transaction BYTEA;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS fee_signed_transaction BYTEA;
ALTER TABLE otc_temp_settlements DROP CONSTRAINT IF EXISTS otc_temp_settlements_state_check;
ALTER TABLE otc_temp_settlements ADD CONSTRAINT otc_temp_settlements_state_check
  CHECK(state IN ('awaiting_payment','payout_claim','payout_submitted','intents_submitted','intents_payout_claim','intents_payout_submitted','intents_refunded','fee_submitted','fee_retained','stalled','settled','failed'));
-- Earlier builds had a narrower state check; widen it in place.
CREATE INDEX IF NOT EXISTS otc_temp_settlements_due
 ON otc_temp_settlements(state, updated_at);
