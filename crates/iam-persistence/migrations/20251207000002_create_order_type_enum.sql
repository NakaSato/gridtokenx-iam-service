-- Create order_type enum
DO $$ BEGIN
    CREATE TYPE order_type AS ENUM ('limit', 'market');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Drop constraint if exists (it might have been dropped already, but good for safety)
ALTER TABLE trading_orders DROP CONSTRAINT IF EXISTS chk_order_type;

-- Convert trading_orders.order_type from VARCHAR to order_type enum
-- We default old values to 'limit' because old schema used 'buy'/'sell' which are now in 'side' column
ALTER TABLE trading_orders 
    ALTER COLUMN order_type TYPE order_type 
    USING ('limit'::order_type);
