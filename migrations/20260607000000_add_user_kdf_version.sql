-- Migration: Add kdf_version to users table
-- Purpose: Track the PBKDF2 KDF version of a user's custodial wallet key so a
--          future lazy re-wrap on auth can upgrade legacy keys
--          (1 = 100k iterations legacy, 2 = 600k). Required by the user
--          repository SELECTs (iam-persistence). Forgotten in commit 08d18ee.

ALTER TABLE users ADD COLUMN IF NOT EXISTS kdf_version SMALLINT NOT NULL DEFAULT 1;

COMMENT ON COLUMN users.kdf_version IS 'PBKDF2 KDF version for the custodial wallet key (1 = 100k legacy, 2 = 600k)';
