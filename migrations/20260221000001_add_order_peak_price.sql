-- Migration: Add last_peak_price for trailing stop logic
-- Description: Adds a column to track the highest/lowest price observed for conditional orders.

ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS last_peak_price DECIMAL(20, 8);

COMMENT ON COLUMN trading_orders.last_peak_price IS 'The highest (for sell) or lowest (for buy) price observed since a trailing stop order was activated';
