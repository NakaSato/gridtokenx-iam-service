-- Migration to add zone_id to order_matches for sharded analytics
-- Created: March 3, 2026

-- Add zone_id column
ALTER TABLE order_matches ADD COLUMN zone_id INTEGER;

-- Create index for faster zone-based queries
CREATE INDEX idx_order_matches_zone_id ON order_matches(zone_id);

-- Update existing records by joining with trading_orders (if any)
-- This ensures data consistency for already matched orders
UPDATE order_matches om
SET zone_id = t.zone_id
FROM trading_orders t
WHERE om.buy_order_id = t.id AND t.zone_id IS NOT NULL;
