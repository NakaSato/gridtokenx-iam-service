-- Add next_retry_at columns for exponential back-off
-- Migration: 20260402000713_add_retry_backoff

-- 1. Update trading_orders
ALTER TABLE trading_orders 
ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ DEFAULT NOW();

-- 2. Update settlements
ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ DEFAULT NOW();

-- 3. Update recurring_order_executions
ALTER TABLE recurring_order_executions
ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ DEFAULT NOW();

-- Add indexes for optimized polling
CREATE INDEX IF NOT EXISTS idx_trading_orders_retry_at ON trading_orders(next_retry_at) WHERE blockchain_status = 'failed_retry';
CREATE INDEX IF NOT EXISTS idx_settlements_retry_at ON settlements(next_retry_at) WHERE blockchain_status = 'failed_retry';
CREATE INDEX IF NOT EXISTS idx_recurring_executions_retry_at ON recurring_order_executions(next_retry_at) WHERE blockchain_status = 'failed_retry';

-- Comments
COMMENT ON COLUMN trading_orders.next_retry_at IS 'Timestamp for the next scheduled blockchain retry';
COMMENT ON COLUMN settlements.next_retry_at IS 'Timestamp for the next scheduled blockchain retry';
COMMENT ON COLUMN recurring_order_executions.next_retry_at IS 'Timestamp for the next scheduled blockchain retry';
