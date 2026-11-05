ALTER TABLE otc_p2p_attempts
 DROP CONSTRAINT IF EXISTS otc_p2p_attempts_chain_check;
ALTER TABLE otc_p2p_attempts
 ADD CONSTRAINT otc_p2p_attempts_chain_check
 CHECK(chain IN ('ethereum','everscale','near-intents'));
