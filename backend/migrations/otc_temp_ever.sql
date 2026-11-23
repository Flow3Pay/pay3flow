-- EVER leg payout for deals whose EVER side is routed through a temporary
-- wallet (spec §6.1 step 11). The USD settlement row now also tracks the
-- message that forwards the funded EVER temp to the payee.
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS ever_state TEXT;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS ever_reference TEXT;
-- Persist the exact signed external message before broadcasting so a lost
-- response can only replay the same Everscale transaction.
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS ever_signed_message BYTEA;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS ever_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE otc_temp_settlements ADD COLUMN IF NOT EXISTS ever_error TEXT;
