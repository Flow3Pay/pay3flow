-- Additive: accepted direct-settlement trades retain their original terms.
CREATE TABLE IF NOT EXISTS otc_escrows (
 trade_id UUID PRIMARY KEY REFERENCES otc_trades(id),
 deployment JSONB NOT NULL, policy JSONB NOT NULL,
 safe TEXT, vault TEXT, deployed_at TIMESTAMPTZ, expires_at TIMESTAMPTZ,
 deployment_block TEXT,
 scan_from TEXT NOT NULL,
 scanned_through TEXT,
 state TEXT NOT NULL DEFAULT 'awaiting_consent' CHECK(state IN
 ('awaiting_consent','created','funded','released','refunded','expired','review')),
 observed_at TIMESTAMPTZ, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(vault), UNIQUE(safe)
);
CREATE TABLE IF NOT EXISTS otc_escrow_consents (
 trade_id UUID NOT NULL REFERENCES otc_escrows(trade_id), owner TEXT NOT NULL,
 signature TEXT NOT NULL, idempotency_key TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 PRIMARY KEY(trade_id,owner), UNIQUE(trade_id,owner,idempotency_key)
);
CREATE TABLE IF NOT EXISTS otc_escrow_proposals (
 id UUID PRIMARY KEY, trade_id UUID NOT NULL REFERENCES otc_escrows(trade_id),
 action TEXT NOT NULL CHECK(action IN ('release','refund_early')),
 nonce TEXT NOT NULL, hash TEXT NOT NULL UNIQUE, typed_data JSONB NOT NULL,
 dispute BOOLEAN NOT NULL DEFAULT FALSE, reason TEXT NOT NULL, evidence JSONB NOT NULL,
 author TEXT NOT NULL, idempotency_key TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(trade_id,author,idempotency_key), UNIQUE(trade_id,nonce,action)
);
CREATE TABLE IF NOT EXISTS otc_escrow_approvals (
 proposal_id UUID NOT NULL REFERENCES otc_escrow_proposals(id), owner TEXT NOT NULL,
 signature TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 PRIMARY KEY(proposal_id,owner)
);
CREATE TABLE IF NOT EXISTS otc_escrow_calls (
 id UUID PRIMARY KEY, trade_id UUID NOT NULL REFERENCES otc_escrows(trade_id),
 actor TEXT NOT NULL, action TEXT NOT NULL, proposal_id UUID REFERENCES otc_escrow_proposals(id),
 sender TEXT NOT NULL, instructions JSONB NOT NULL, idempotency_key TEXT NOT NULL,
 reference TEXT, state TEXT NOT NULL DEFAULT 'unknown' CHECK(state IN ('unknown','submitted','verified','failed')),
 evidence JSONB, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 scan_cursor TEXT,
 UNIQUE(trade_id,actor,idempotency_key)
);
CREATE UNIQUE INDEX IF NOT EXISTS otc_escrow_wallet_handoff ON otc_escrow_calls(sender)
 WHERE state IN ('unknown','submitted');
ALTER TABLE otc_escrows ADD COLUMN IF NOT EXISTS scanned_through TEXT;
ALTER TABLE otc_escrow_calls ADD COLUMN IF NOT EXISTS scan_cursor TEXT;
CREATE TABLE IF NOT EXISTS otc_escrow_evidence (
 trade_id UUID NOT NULL REFERENCES otc_escrows(trade_id), kind TEXT NOT NULL,
 identity TEXT NOT NULL UNIQUE, evidence JSONB NOT NULL, included_at TIMESTAMPTZ NOT NULL,
 valid BOOLEAN NOT NULL DEFAULT TRUE, last_checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 PRIMARY KEY(trade_id,kind)
);

CREATE OR REPLACE FUNCTION otc_escrow_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.deployment IS DISTINCT FROM OLD.deployment OR NEW.policy IS DISTINCT FROM OLD.policy
    OR NEW.trade_id IS DISTINCT FROM OLD.trade_id OR NEW.scan_from IS DISTINCT FROM OLD.scan_from THEN
  RAISE EXCEPTION 'Escrow deployment consent and policy are immutable';
 END IF;
 RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS otc_escrow_immutable ON otc_escrows;
CREATE TRIGGER otc_escrow_immutable BEFORE UPDATE ON otc_escrows
 FOR EACH ROW EXECUTE FUNCTION otc_escrow_immutable();
