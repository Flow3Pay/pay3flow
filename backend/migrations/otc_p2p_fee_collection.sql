ALTER TABLE otc_p2p_attempts
 DROP CONSTRAINT IF EXISTS otc_p2p_attempts_kind_check;
ALTER TABLE otc_p2p_attempts
 ADD CONSTRAINT otc_p2p_attempts_kind_check
 CHECK(kind IN ('payment','payout','fee'));
