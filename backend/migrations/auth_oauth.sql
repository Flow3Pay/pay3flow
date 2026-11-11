-- Phase 2 email-code auth (docs/b2b-merchant-portal.md §4.3, §4.4).
--
-- Codes are six digits, live for ten minutes and are stored as an Argon2id
-- hash. One row per email: a resend replaces the previous code. Send history is
-- kept separately so the one-per-minute and five-per-hour limits survive
-- process restarts.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS auth_codes (
    email TEXT PRIMARY KEY,
    code_hash TEXT NOT NULL,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    attempts SMALLINT NOT NULL DEFAULT 0 CHECK (attempts >= 0)
);

CREATE INDEX IF NOT EXISTS auth_codes_expires_idx ON auth_codes (expires_at);

-- Send attempts per email and IP. Only timestamps are kept; the repository
-- prunes rows older than a day whenever a new code is issued.
CREATE TABLE IF NOT EXISTS auth_code_sends (
    id BIGSERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    ip TEXT,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS auth_code_sends_email_idx
    ON auth_code_sends (email, sent_at DESC);
CREATE INDEX IF NOT EXISTS auth_code_sends_ip_idx
    ON auth_code_sends (ip, sent_at DESC);
