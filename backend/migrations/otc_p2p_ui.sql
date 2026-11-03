-- Mutual hand-off confirmation and deterministic wallet avatars for the P2P workflow.
ALTER TABLE otc_p2p_trades ADD COLUMN IF NOT EXISTS owner_confirmed_at TIMESTAMPTZ;
ALTER TABLE otc_p2p_trades ADD COLUMN IF NOT EXISTS taker_confirmed_at TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS otc_avatars (
 chain TEXT NOT NULL CHECK(chain IN ('ethereum','everscale')),
 address TEXT NOT NULL,
 public_key TEXT,
 avatar TEXT NOT NULL,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 PRIMARY KEY(chain,address)
);
