-- Re-seed the dev "Engineering Default Key" after switching ApiKeyService::hash_key
-- from a non-standard SHA-256(key || secret) digest to HMAC-SHA256(secret, key).
-- The hash format changed, so the value written by 20260626000000 no longer
-- matches what live IAM now computes; without this the well-known dev key would
-- 401 again (bridge ingest would fall back to its static GRIDTOKENX_API_KEYS).
--
-- key_hash = hash_key() = lowercase-hex HMAC-SHA256(API_KEY_SECRET, key_bytes)
--   (gridtokenx-iam-service/crates/iam-logic/src/jwt_service.rs::hash_key)
--     key    = "engineering-department-api-key-2025"
--     secret = the dev API_KEY_SECRET pinned in .env / .env.example
--              ("test-api-key-secret-for-development-and-testing")
--
-- DEV-ONLY: a deployment that sets a different API_KEY_SECRET (or registered its
-- own keys) must re-register via ApiKeyService — those rows simply will not match
-- the old SHA-256 hash and so are left untouched here.
--
-- Separate migration (not an edit of 20260626000000) so the already-applied
-- migration's checksum stays intact; the guarded WHERE makes it idempotent.
UPDATE api_keys
SET key_hash = '0c9b5d31c7e6ec3963f5c7de72c4a6d3346a2991b7021e2f70e7392b5662ac21',
    is_active = true
WHERE name = 'Engineering Default Key'
  AND key_hash = 'f3d1e617a08a1dc72b45ce665a28348f7361e551e795c9635dc9f8160e7d2ef0';
