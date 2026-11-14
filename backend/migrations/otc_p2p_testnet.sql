ALTER TABLE otc_p2p_trades
 DROP CONSTRAINT IF EXISTS otc_p2p_trades_state_check;
ALTER TABLE otc_p2p_trades
 ADD CONSTRAINT otc_p2p_trades_state_check
 CHECK(state IN ('payment_pending','delivery_pending','completed','testnet_completed','expired','cancelled'));

ALTER TABLE otc_events
 ADD COLUMN IF NOT EXISTS p2p_trade_id UUID REFERENCES otc_p2p_trades(id);
CREATE INDEX IF NOT EXISTS otc_events_p2p_trade_recent
 ON otc_events(p2p_trade_id, created_at DESC);
