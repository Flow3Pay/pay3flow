-- Native EVER agreements are independent of desk settlement accounting.
CREATE TABLE IF NOT EXISTS otc_ever_agreements (
 address TEXT PRIMARY KEY, terms_hash TEXT NOT NULL UNIQUE, manifest JSONB NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS otc_ever_calls (
 id UUID PRIMARY KEY, address TEXT NOT NULL REFERENCES otc_ever_agreements(address),
 actor TEXT NOT NULL, public_key TEXT NOT NULL, terms_hash TEXT NOT NULL,
 instructions JSONB NOT NULL, idempotency_key TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'unknown' CHECK (state IN ('unknown','accepted','failed')),
 reference TEXT UNIQUE, evidence JSONB, created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 UNIQUE(actor,idempotency_key)
);
CREATE UNIQUE INDEX IF NOT EXISTS otc_ever_unknown_call ON otc_ever_calls(address)
 WHERE state='unknown';

CREATE OR REPLACE FUNCTION otc_ever_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_TABLE_NAME='otc_ever_agreements' THEN
  IF NEW IS DISTINCT FROM OLD THEN RAISE EXCEPTION 'immutable EVER agreement'; END IF;
 ELSE
  IF (NEW.id,NEW.address,NEW.actor,NEW.public_key,NEW.terms_hash,NEW.instructions,NEW.idempotency_key,NEW.created_at)
     IS DISTINCT FROM
     (OLD.id,OLD.address,OLD.actor,OLD.public_key,OLD.terms_hash,OLD.instructions,OLD.idempotency_key,OLD.created_at)
     OR (OLD.state<>'unknown' AND NEW IS DISTINCT FROM OLD)
  THEN RAISE EXCEPTION 'immutable EVER handoff'; END IF;
 END IF;
 RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS otc_ever_agreement_immutable ON otc_ever_agreements;
CREATE TRIGGER otc_ever_agreement_immutable BEFORE UPDATE ON otc_ever_agreements
 FOR EACH ROW EXECUTE FUNCTION otc_ever_immutable();
DROP TRIGGER IF EXISTS otc_ever_call_immutable ON otc_ever_calls;
CREATE TRIGGER otc_ever_call_immutable BEFORE UPDATE ON otc_ever_calls
 FOR EACH ROW EXECUTE FUNCTION otc_ever_immutable();
