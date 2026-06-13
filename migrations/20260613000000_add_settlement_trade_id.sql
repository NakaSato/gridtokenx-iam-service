-- Persist settlements.trade_id so trade (matching-engine) settlements are
-- distinguishable from oracle/generation settlements (oracle = NULL).
-- Previously trade_id was never stored, so every settlement read back as
-- trade_id = NULL and the settlement engine minted it as oracle surplus
-- generation instead of treating it as a trade.
ALTER TABLE settlements ADD COLUMN IF NOT EXISTS trade_id UUID;

-- Allow the terminal `permanently_failed` status the trading service writes
-- once a settlement exhausts its retry budget. The original constraint only
-- permitted up to `failed`, which would make that UPDATE fail at runtime.
ALTER TABLE settlements DROP CONSTRAINT IF EXISTS chk_settlement_status;
ALTER TABLE settlements ADD CONSTRAINT chk_settlement_status
    CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'permanently_failed'));
