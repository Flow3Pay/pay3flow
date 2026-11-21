-- Custodial temporary intermediaries for OTC deals (spec §5.3). Key material,
-- salted encryption, EVM deal wiring and settlement signing live in
-- `otc::temp_wallet` / `otc::temp_settle`; no row exists while
-- `OTC_TEMP_WALLETS` is off.
CREATE TABLE IF NOT EXISTS otc_temp_wallets (
 id UUID PRIMARY KEY,
 -- P2P acceptance is the only deal flow that will create temps in this codebase,
 -- so the deal reference points at otc_p2p_trades rather than otc_trades.
 trade_id UUID REFERENCES otc_p2p_trades(id),
 role TEXT NOT NULL CHECK(role IN ('ever','usd','usd_in','usd_out')),
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
-- Attempt rows link the custody row that received the deposit (spec §6.1 step
-- 12). Nullable: legacy and non-custodial attempts have no temporary wallet.
ALTER TABLE otc_p2p_attempts ADD COLUMN IF NOT EXISTS temp_wallet_id UUID REFERENCES otc_temp_wallets(id);
CREATE INDEX IF NOT EXISTS otc_p2p_attempts_temp_wallet
 ON otc_p2p_attempts(temp_wallet_id) WHERE temp_wallet_id IS NOT NULL;
