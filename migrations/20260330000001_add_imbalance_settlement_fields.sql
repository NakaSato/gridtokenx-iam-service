-- Migration: Add imbalance settlement fields to settlements table
-- Purpose: Support financial corrections for energy volume discrepancies

-- 1. Add new columns
ALTER TABLE settlements ADD COLUMN IF NOT EXISTS settlement_type VARCHAR(30) DEFAULT 'standard';
ALTER TABLE settlements ADD COLUMN IF NOT EXISTS imbalance_gap_kwh NUMERIC(20, 8);
ALTER TABLE settlements ADD COLUMN IF NOT EXISTS imbalance_price NUMERIC(20, 8);

-- 2. Add check constraint for settlement_type
ALTER TABLE settlements DROP CONSTRAINT IF EXISTS chk_settlement_type;
ALTER TABLE settlements ADD CONSTRAINT chk_settlement_type 
    CHECK (settlement_type IN ('standard', 'imbalance_correction', 'vpp_reserve_credit', 'grid_penalty'));

-- 3. Make order correlations optional for corrections
-- Standard settlements are tied to matches, but imbalances are aggregated per user/epoch.
ALTER TABLE settlements ALTER COLUMN buy_order_id DROP NOT NULL;
ALTER TABLE settlements ALTER COLUMN sell_order_id DROP NOT NULL;

-- 4. Add index for type-based reporting
CREATE INDEX IF NOT EXISTS idx_settlements_type ON settlements(settlement_type);

COMMENT ON COLUMN settlements.settlement_type IS 'Distinguishes between trade-based and physical-correction settlements';
COMMENT ON COLUMN settlements.imbalance_gap_kwh IS 'The physical energy difference being settled (Metered - Comitted)';
