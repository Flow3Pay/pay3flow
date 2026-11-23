-- Automatic USDT->native gas top-ups for OTC treasury wallets.
--
-- When the treasury native balance on any configured EVM network drops below
-- OTC_TEMP_GAS_LOW_THRESHOLD the settlement worker opens an exact-output NEAR
-- Intents swap that delivers OTC_TEMP_GAS_TARGET worth of native token back to
-- the treasury address.  One active row per chain at a time; completed and
-- failed rows are kept for the audit trail.
CREATE TABLE IF NOT EXISTS otc_temp_gas_topups (
 id UUID PRIMARY KEY,
 chain TEXT NOT NULL,
 treasury_address TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'claim'
   CHECK(state IN ('claim','submitted','intents_submitted','completed','failed')),
 -- EVM reference for the USDT->Intents-deposit ERC-20 transfer.
 reference TEXT,
 signed_transaction BYTEA,
 -- NEAR Intents deposit and routing details persisted before the first poll.
 deposit_address TEXT,
 deposit_memo TEXT,
 quote_id TEXT,
 -- Atomic USDT units sent to the Intents deposit address.
 source_units TEXT,
 -- Native-token units (wei) we requested back.
 target_units TEXT,
 intents_started_at TIMESTAMPTZ,
 attempts INTEGER NOT NULL DEFAULT 0,
 last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Fast lookup of the latest active row for each chain.
CREATE INDEX IF NOT EXISTS otc_temp_gas_topups_due
 ON otc_temp_gas_topups(chain, state, updated_at);
