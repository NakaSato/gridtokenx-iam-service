-- Add filled_at column to trading_orders table
ALTER TABLE trading_orders ADD COLUMN IF NOT EXISTS filled_at TIMESTAMPTZ;
