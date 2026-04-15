-- Add is_confidential column to trading_orders
ALTER TABLE trading_orders ADD COLUMN IF NOT EXISTS is_confidential BOOLEAN DEFAULT FALSE;
