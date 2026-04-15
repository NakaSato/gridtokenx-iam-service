-- Add zone_id to trading_orders and cost breakdown to settlements
-- Created: 2025-12-31

-- 1. Update trading_orders table
ALTER TABLE trading_orders ADD COLUMN IF NOT EXISTS zone_id INTEGER;

-- 2. Update settlements table with detailed cost breakdown
ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS wheeling_charge NUMERIC(20, 8) DEFAULT 0;

ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS loss_factor NUMERIC(20, 8) DEFAULT 0;

ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS loss_cost NUMERIC(20, 8) DEFAULT 0;

ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS effective_energy NUMERIC(20, 8) DEFAULT 0;

ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS buyer_zone_id INTEGER;

ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS seller_zone_id INTEGER;

-- 3. Add comment for clarity
COMMENT ON COLUMN settlements.wheeling_charge IS 'Zone-based transmission fee (THB)';

COMMENT ON COLUMN settlements.loss_factor IS 'Technical loss percentage (decimal)';

COMMENT ON COLUMN settlements.loss_cost IS 'Monetized energy loss (THB)';

COMMENT ON COLUMN settlements.effective_energy IS 'Energy received by buyer after losses (kWh)';