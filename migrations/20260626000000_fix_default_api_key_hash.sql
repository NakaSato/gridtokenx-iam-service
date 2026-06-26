-- Replace the placeholder hash on the dev "Engineering Default Key" with the real one,
-- so the well-known key works under LIVE IAM — not only via the Aggregator Bridge's
-- static GRIDTOKENX_API_KEYS fallback (which engages only when the IAM gRPC client is
-- unreachable). The seed migration (20260325000002) shipped key_hash = SHA-256("") as a
-- placeholder, so live IAM rejected the key and bridge ingest returned 401.
--
-- key_hash = hash_key() = lowercase-hex SHA-256(key_bytes ++ API_KEY_SECRET_bytes)
--   (gridtokenx-iam-service/crates/iam-logic/src/jwt_service.rs::hash_key)
--     key    = "engineering-department-api-key-2025"
--     secret = the dev API_KEY_SECRET pinned in .env / .env.example
--              ("test-api-key-secret-for-development-and-testing")
--
-- DEV-ONLY: a deployment that sets a different API_KEY_SECRET must register its own key
-- via ApiKeyService — this row simply will not match and stays inert.
--
-- Done as a separate migration (not an edit of 20260325000002) so the checksum of the
-- already-applied seed migration stays intact; the guarded WHERE makes it idempotent and
-- avoids clobbering a hash already corrected out-of-band.
UPDATE api_keys
SET key_hash = 'f3d1e617a08a1dc72b45ce665a28348f7361e551e795c9635dc9f8160e7d2ef0',
    is_active = true
WHERE name = 'Engineering Default Key'
  AND key_hash = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855';
