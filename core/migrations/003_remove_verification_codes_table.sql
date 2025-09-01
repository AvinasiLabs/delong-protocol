-- Remove verification_codes table as we're using Redis for verification codes
DROP INDEX IF EXISTS idx_verification_codes_email;
DROP INDEX IF EXISTS idx_verification_codes_type;
DROP INDEX IF EXISTS idx_verification_codes_expires_at;
DROP INDEX IF EXISTS idx_verification_codes_is_used;

DROP TABLE IF EXISTS verification_codes CASCADE;
