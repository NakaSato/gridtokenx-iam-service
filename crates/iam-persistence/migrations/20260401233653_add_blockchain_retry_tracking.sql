-- Add blockchain tracking and retry columns to support robust on-chain operations
-- Migration: 20260401233653_add_blockchain_retry_tracking

-- 1. Update trading_orders
ALTER TABLE trading_orders 
ADD COLUMN IF NOT EXISTS blockchain_status VARCHAR(20) DEFAULT 'unprocessed',
ADD COLUMN IF NOT EXISTS blockchain_tx_hash VARCHAR(88),
ADD COLUMN IF NOT EXISTS blockchain_error TEXT,
ADD COLUMN IF NOT EXISTS retry_count INT DEFAULT 0;

-- 2. Update settlements
ALTER TABLE settlements
ADD COLUMN IF NOT EXISTS blockchain_status VARCHAR(20) DEFAULT 'unprocessed',
ADD COLUMN IF NOT EXISTS blockchain_tx_hash VARCHAR(88),
ADD COLUMN IF NOT EXISTS blockchain_error TEXT,
ADD COLUMN IF NOT EXISTS retry_count INT DEFAULT 0;

-- 3. Update recurring_order_executions
ALTER TABLE recurring_order_executions
ADD COLUMN IF NOT EXISTS blockchain_status VARCHAR(20) DEFAULT 'unprocessed',
ADD COLUMN IF NOT EXISTS blockchain_tx_hash VARCHAR(88),
ADD COLUMN IF NOT EXISTS blockchain_error TEXT,
ADD COLUMN IF NOT EXISTS retry_count INT DEFAULT 0;

-- Add indexes for performance when polling for retries
CREATE INDEX IF NOT EXISTS idx_trading_orders_blockchain_status ON trading_orders(blockchain_status) WHERE blockchain_status != 'success';
CREATE INDEX IF NOT EXISTS idx_settlements_blockchain_status ON settlements(blockchain_status) WHERE blockchain_status != 'success';
CREATE INDEX IF NOT EXISTS idx_recurring_executions_blockchain_status ON recurring_order_executions(blockchain_status) WHERE blockchain_status != 'success';

-- Comments
COMMENT ON COLUMN trading_orders.blockchain_status IS 'On-chain processing status: unprocessed, pending, success, failed_retry, failed_fatal';
COMMENT ON COLUMN settlements.blockchain_status IS 'On-chain processing status: unprocessed, pending, success, failed_retry, failed_fatal';
COMMENT ON COLUMN recurring_order_executions.blockchain_status IS 'On-chain processing status: unprocessed, pending, success, failed_retry, failed_fatal';
