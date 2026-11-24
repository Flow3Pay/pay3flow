-- Custodial temporary intermediaries for OTC deals (spec §5.3). Key material,
-- salted encryption, EVM deal wiring and settlement signing live in
-- `otc::temp_wallet` / `otc::temp_settle`; no row exists while
-- `OTC_TEMP_WALLETS` is off.
CREATE TABLE IF NOT EXISTS otc_temp_wallets (
 id UUID PRIMARY KEY,
 -- P2P acceptance is the only deal flow that will create temps in this codebase,
 -- so the deal reference points at otc_p2p_trades rather than otc_trades.
 trade_id UUID REFERENCES otc_p2p_trades(id),
 role TEXT NOT NULL CHECK(role IN ('ever','usd','usd_in','usd_out','reserve')),
 chain TEXT NOT NULL,
 address TEXT NOT NULL,
 derivation_index INTEGER NOT NULL CHECK(derivation_index >= 0),
 salt BYTEA NOT NULL CHECK(octet_length(salt) >= 16),
 key_ciphertext BYTEA NOT NULL CHECK(octet_length(key_ciphertext) > 12),
 status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','recycled','deleted')),
 parent_wallet_id UUID REFERENCES otc_temp_wallets(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS otc_temp_wallets_trade_role
 ON otc_temp_wallets(trade_id, role) WHERE trade_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS otc_temp_wallets_address
 ON otc_temp_wallets(chain, address);
CREATE INDEX IF NOT EXISTS otc_temp_wallets_pool
 ON otc_temp_wallets(chain, status) WHERE trade_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS otc_temp_wallets_child
 ON otc_temp_wallets(parent_wallet_id, role) WHERE parent_wallet_id IS NOT NULL;
-- Same-network payouts can be fronted from one platform reserve per chain. A
-- previous build restricted the role list, so widen it in place. The treasury
-- role is added here too so reruns after otc_temp_treasury.sql do not violate
-- existing treasury rows.
ALTER TABLE otc_temp_wallets DROP CONSTRAINT IF EXISTS otc_temp_wallets_role_check;
ALTER TABLE otc_temp_wallets ADD CONSTRAINT otc_temp_wallets_role_check
 CHECK(role IN ('ever','usd','usd_in','usd_out','reserve','treasury'));
CREATE UNIQUE INDEX IF NOT EXISTS otc_temp_wallets_reserve
 ON otc_temp_wallets(chain) WHERE role='reserve' AND trade_id IS NULL;
-- Attempt rows link the custody row that received the deposit (spec §6.1 step
-- 12). Nullable: legacy and non-custodial attempts have no temporary wallet.
ALTER TABLE otc_p2p_attempts ADD COLUMN IF NOT EXISTS temp_wallet_id UUID REFERENCES otc_temp_wallets(id);
CREATE INDEX IF NOT EXISTS otc_p2p_attempts_temp_wallet
 ON otc_p2p_attempts(temp_wallet_id) WHERE temp_wallet_id IS NOT NULL;
-- Attempts created before the column existed still name the temp address in
-- their leg; link those rows once so the join works for historical deals too.
UPDATE otc_p2p_attempts a SET temp_wallet_id = w.id
 FROM otc_temp_wallets w
 WHERE a.temp_wallet_id IS NULL AND a.kind = 'payment'
   AND a.trade_id = w.trade_id
   AND a.instructions->'leg'->>'sender' = w.address;
