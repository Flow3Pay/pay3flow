ALTER TABLE otc_p2p_fees ADD COLUMN IF NOT EXISTS payer_actor TEXT;

UPDATE otc_p2p_fees fee
SET payer_actor = CASE
    WHEN trade.body->>'direction' = 'buy' THEN trade.owner_actor
    ELSE trade.taker_actor
END
FROM otc_p2p_trades trade
WHERE fee.trade_id = trade.id
  AND fee.payer_actor IS NULL;

ALTER TABLE otc_p2p_fees ALTER COLUMN payer_actor SET NOT NULL;
CREATE INDEX IF NOT EXISTS otc_p2p_fees_payer ON otc_p2p_fees(payer_actor, created_at DESC);
