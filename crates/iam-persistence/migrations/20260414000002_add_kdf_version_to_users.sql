-- Migration: Add KDF version to users table
-- Purpose: Track which PBKDF2 iteration profile encrypted a user's custodial
--          wallet key, so records can be lazily re-wrapped from v1 (100k) to
--          v2 (600k, OWASP-2023) on successful auth without a flag-day.
--          See gridtokenx-blockchain-core wallet::CURRENT_KDF_VERSION /
--          needs_rewrap and REVIEW_FINDINGS.md P4.
-- NOTE: forward-prep only. IAM does not yet exercise the custodial
--       encrypt/decrypt path; the re-wrap step is wired when it does.

-- 1. Add column. Default 1 = existing records were encrypted with v1 (100k).
ALTER TABLE users ADD COLUMN IF NOT EXISTS kdf_version SMALLINT NOT NULL DEFAULT 1;

-- 2. Comment
COMMENT ON COLUMN users.kdf_version IS 'PBKDF2 KDF version for the custodial wallet key: 1 = 100k iters (legacy), 2 = 600k iters. Lazily upgraded on auth.';
