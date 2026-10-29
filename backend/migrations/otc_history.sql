-- Chart history is aggregated on read from the desk's own prints, so the only
-- thing it needs is a time-ordering index on the trades table. Public offers and
-- p2p deals already carry otc_*_recent indexes on created_at.
CREATE INDEX IF NOT EXISTS otc_trades_recent ON otc_trades(created_at DESC);
