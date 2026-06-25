-- Chain Bridge tamper-evident audit trail (Gap #2) — shared-DB owner copy.
--
-- The platform DB schema is owned by THIS (IAM) migration runner. Chain Bridge
-- has no runner of its own, so its audit table is folded in here for
-- reproducible deploys. Source: gridtokenx-chain-bridge/migrations/0001_audit_log.sql.
--
-- Hash-chained: each row's entry_hash = SHA-256(prev_hash || canonical fields),
-- so any retroactive edit breaks every later link. Written by
-- chain_bridge_persistence::PostgresAuditStore on every mediated signing
-- decision (policy/auth reject, submit) in ChainBridgeGrpcService::sign_and_submit.
-- Idempotent so it is safe alongside any prior out-of-band application.

CREATE TABLE IF NOT EXISTS audit_log (
    id             BIGSERIAL PRIMARY KEY,
    prev_hash      BYTEA,                       -- NULL only for the genesis row
    entry_hash     BYTEA       NOT NULL,        -- 32-byte SHA-256 chain link
    correlation_id TEXT        NOT NULL DEFAULT '',
    identity       TEXT        NOT NULL,        -- SPIFFE id of the caller
    action         TEXT        NOT NULL,        -- e.g. 'sign_and_submit'
    outcome_json   TEXT        NOT NULL,        -- serialized AuditOutcome
    created_at_ms  BIGINT      NOT NULL         -- producer-stamped epoch millis
);

-- Tip lookup on append (ORDER BY id DESC LIMIT 1) is served by the PK.
CREATE INDEX IF NOT EXISTS idx_audit_log_correlation_id ON audit_log (correlation_id);
