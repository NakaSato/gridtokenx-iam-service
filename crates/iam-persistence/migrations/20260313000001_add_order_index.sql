-- Add order_index column to trading_orders table
-- This stores the on-chain active_orders counter at the time of creation
-- Used for re-deriving the Order PDA if needed

ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS order_index BIGINT;

-- Link logic: Program expects u64, BIGINT in PG is 8 bytes.
