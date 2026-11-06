-- The fee is always owed in USDT; USDC deals use a nominal 1:1 conversion.
-- Keep the original token alongside the fee so the accrual stays auditable.
ALTER TABLE otc_p2p_fees ADD COLUMN IF NOT EXISTS basis_asset TEXT NOT NULL DEFAULT 'USDT';

UPDATE otc_p2p_fees fee
SET basis_asset = CASE
    WHEN trade.body->>'stablecoin' = 'USDC' THEN 'USDC'
    ELSE 'USDT'
END
FROM otc_p2p_trades trade
WHERE fee.trade_id = trade.id;

ALTER TABLE otc_p2p_fees DROP CONSTRAINT IF EXISTS otc_p2p_fees_basis_asset_check;
ALTER TABLE otc_p2p_fees
 ADD CONSTRAINT otc_p2p_fees_basis_asset_check CHECK(basis_asset IN ('USDT','USDC'));
