-- OTC is isolated from the legacy acquiring/exchange simulation ledger.
CREATE TABLE IF NOT EXISTS otc_sessions (
 id UUID PRIMARY KEY, actor TEXT NOT NULL, credential TEXT NOT NULL,
 expires_at TIMESTAMPTZ NOT NULL, revoked BOOLEAN NOT NULL DEFAULT FALSE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS otc_wallet_challenges (
 id UUID PRIMARY KEY, session_id UUID NOT NULL REFERENCES otc_sessions(id),
 chain TEXT NOT NULL CHECK(chain IN ('ethereum','everscale')), address TEXT NOT NULL,
 message TEXT NOT NULL, expires_at TIMESTAMPTZ NOT NULL, consumed_at TIMESTAMPTZ
);
CREATE TABLE IF NOT EXISTS otc_wallets (
 actor TEXT NOT NULL, chain TEXT NOT NULL, address TEXT NOT NULL, verified_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(actor,chain)
);
CREATE TABLE IF NOT EXISTS otc_rfq (
 id UUID PRIMARY KEY, actor TEXT NOT NULL, direction TEXT NOT NULL CHECK(direction IN ('buy','sell')),
 input TEXT NOT NULL, listing_id TEXT NOT NULL, body JSONB NOT NULL, state TEXT NOT NULL DEFAULT 'waiting',
 idempotency_key TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(actor,idempotency_key)
);
CREATE TABLE IF NOT EXISTS otc_trades (
 id UUID PRIMARY KEY, rfq_id UUID NOT NULL REFERENCES otc_rfq(id),
 customer_actor TEXT NOT NULL, desk_actor TEXT NOT NULL, terms JSONB NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('quoted','booking_pending','accepted','payment_confirming','funded','payout_pending','completed','rejected','expired','review','refund_pending','refunded')),
 offer JSONB, decision JSONB, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(rfq_id)
);
CREATE TABLE IF NOT EXISTS otc_inventory (
 chain TEXT PRIMARY KEY CHECK(chain IN ('ethereum','everscale')),
 balance NUMERIC(39,0) NOT NULL CHECK(balance>=0), gas_reserve NUMERIC(39,0) NOT NULL CHECK(gas_reserve>=0),
 observed_at TIMESTAMPTZ NOT NULL, snapshot JSONB NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS otc_reservations (
 trade_id UUID PRIMARY KEY REFERENCES otc_trades(id), chain TEXT NOT NULL REFERENCES otc_inventory(chain),
 amount NUMERIC(39,0) NOT NULL CHECK(amount>0), status TEXT NOT NULL CHECK(status IN ('active','debit','released')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS otc_attempts (
 id UUID PRIMARY KEY, trade_id UUID NOT NULL REFERENCES otc_trades(id),
 kind TEXT NOT NULL CHECK(kind IN ('payment','payout','refund')), chain TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('prepared','unknown','submitted','confirming','verified','failed')),
 instructions JSONB NOT NULL, reference TEXT, idempotency_key TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(trade_id,kind,idempotency_key)
);
CREATE UNIQUE INDEX IF NOT EXISTS otc_one_unresolved_attempt ON otc_attempts(trade_id,kind)
 WHERE state IN ('prepared','unknown','submitted','confirming');
CREATE TABLE IF NOT EXISTS otc_evidence (
 chain TEXT NOT NULL, identity TEXT NOT NULL, trade_id UUID NOT NULL REFERENCES otc_trades(id),
 kind TEXT NOT NULL, evidence JSONB NOT NULL, included_at TIMESTAMPTZ NOT NULL,
 valid BOOLEAN NOT NULL DEFAULT TRUE, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 PRIMARY KEY(chain,identity), UNIQUE(trade_id,kind)
);
CREATE TABLE IF NOT EXISTS otc_ledger (
 id UUID PRIMARY KEY, trade_id UUID NOT NULL REFERENCES otc_trades(id),
 kind TEXT NOT NULL, currency TEXT NOT NULL, amount NUMERIC(39,0) NOT NULL,
 policy JSONB NOT NULL, authorized_by TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS otc_fee_once ON otc_ledger(trade_id) WHERE kind='fee_accrued';
CREATE TABLE IF NOT EXISTS otc_events (
 id BIGSERIAL PRIMARY KEY, trade_id UUID REFERENCES otc_trades(id), rfq_id UUID REFERENCES otc_rfq(id),
 kind TEXT NOT NULL, body JSONB NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS otc_outbox (
 activity_id TEXT PRIMARY KEY, actor TEXT NOT NULL, body JSONB NOT NULL, purpose TEXT NOT NULL,
 delivered_at TIMESTAMPTZ, next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 lease_until TIMESTAMPTZ, attempts INTEGER NOT NULL DEFAULT 0, created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS otc_inbox (
 actor TEXT NOT NULL, activity_id TEXT NOT NULL, body JSONB NOT NULL,
 processed_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY(actor,activity_id)
);
CREATE TABLE IF NOT EXISTS otc_relay_cursors (actor TEXT PRIMARY KEY, sequence BIGINT NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS otc_observers (
 chain TEXT PRIMARY KEY, checkpoint JSONB NOT NULL DEFAULT '{}', last_scan TIMESTAMPTZ,
 failures INTEGER NOT NULL DEFAULT 0, lag BIGINT, paused BOOLEAN NOT NULL DEFAULT TRUE,
 last_error TEXT
);
INSERT INTO otc_observers(chain) VALUES ('ethereum'),('everscale') ON CONFLICT DO NOTHING;
CREATE TABLE IF NOT EXISTS otc_incidents (
 incident_key TEXT PRIMARY KEY, severity TEXT NOT NULL CHECK(severity IN ('P0','P1','P2')),
 owner TEXT NOT NULL, backup TEXT NOT NULL, trade_id UUID REFERENCES otc_trades(id),
 runbook TEXT NOT NULL, evidence JSONB NOT NULL, actions JSONB NOT NULL DEFAULT '[]',
 customer_updates JSONB NOT NULL DEFAULT '[]', detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 acknowledged_at TIMESTAMPTZ, service_restored_at TIMESTAMPTZ, financially_resolved_at TIMESTAMPTZ,
 next_update_at TIMESTAMPTZ, next_notification_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 notifications INTEGER NOT NULL DEFAULT 0, closed_at TIMESTAMPTZ
);
CREATE TABLE IF NOT EXISTS otc_controls (singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton), paused BOOLEAN NOT NULL DEFAULT TRUE);
INSERT INTO otc_controls(singleton) VALUES(TRUE) ON CONFLICT DO NOTHING;
CREATE TABLE IF NOT EXISTS otc_actor_links (
 actor TEXT PRIMARY KEY, credential TEXT NOT NULL, revoked BOOLEAN NOT NULL DEFAULT FALSE,
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE OR REPLACE FUNCTION otc_terms_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.terms IS DISTINCT FROM OLD.terms OR NEW.customer_actor IS DISTINCT FROM OLD.customer_actor OR NEW.desk_actor IS DISTINCT FROM OLD.desk_actor THEN
  RAISE EXCEPTION 'OTC accepted terms and participants are immutable';
 END IF;
 RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS otc_terms_immutable ON otc_trades;
CREATE TRIGGER otc_terms_immutable BEFORE UPDATE ON otc_trades FOR EACH ROW EXECUTE FUNCTION otc_terms_immutable();

ALTER TABLE otc_controls ADD COLUMN IF NOT EXISTS signing_paused BOOLEAN NOT NULL DEFAULT FALSE;
CREATE UNIQUE INDEX IF NOT EXISTS otc_accounting_idempotency ON otc_ledger(trade_id,(policy->>'idempotency_key')) WHERE policy ? 'idempotency_key';

ALTER TABLE otc_attempts ADD COLUMN IF NOT EXISTS failure_evidence JSONB;
ALTER TABLE otc_attempts ADD COLUMN IF NOT EXISTS retry_authorized BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE otc_outbox ADD COLUMN IF NOT EXISTS queued_at TIMESTAMPTZ;

ALTER TABLE otc_controls ADD COLUMN IF NOT EXISTS demo BOOLEAN;

ALTER TABLE otc_evidence ADD COLUMN IF NOT EXISTS last_checked_at TIMESTAMPTZ;

ALTER TABLE otc_attempts DROP CONSTRAINT IF EXISTS otc_attempts_state_check;
ALTER TABLE otc_attempts ADD CONSTRAINT otc_attempts_state_check CHECK(state IN ('prepared','unknown','submitted','confirming','verified','failed','review'));
