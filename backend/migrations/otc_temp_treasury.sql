-- Stable, platform-owned EVM treasury wallets. Existing reserve wallets
-- become treasuries in place so their keys and balances are preserved.
ALTER TABLE otc_temp_wallets DROP CONSTRAINT IF EXISTS otc_temp_wallets_role_check;
ALTER TABLE otc_temp_wallets ADD CONSTRAINT otc_temp_wallets_role_check
 CHECK(role IN ('ever','usd','usd_in','usd_out','reserve','treasury'));
DROP INDEX IF EXISTS otc_temp_wallets_reserve;
UPDATE otc_temp_wallets SET role='treasury'
 WHERE role='reserve' AND trade_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS otc_temp_wallets_treasury
 ON otc_temp_wallets(chain) WHERE role='treasury' AND trade_id IS NULL;
