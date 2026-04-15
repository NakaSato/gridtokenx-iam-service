-- Add conditional order fields for Stop-Loss/Take-Profit functionality
-- Migration: 20260108070356_add_conditional_orders

-- Add trigger_type enum
DO $$ BEGIN
    CREATE TYPE trigger_type AS ENUM ('stop_loss', 'take_profit', 'trailing_stop');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add trigger_status enum
DO $$ BEGIN
    CREATE TYPE trigger_status AS ENUM ('pending', 'triggered', 'cancelled', 'expired');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add conditional order columns to trading_orders
ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS trigger_price DECIMAL(20, 8);

ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS trigger_type trigger_type;

ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS trigger_status trigger_status DEFAULT 'pending';

ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS triggered_at TIMESTAMPTZ;

ALTER TABLE trading_orders
ADD COLUMN IF NOT EXISTS trailing_offset DECIMAL(20, 8);
-- For trailing stop orders

-- Create index for efficient price monitoring
CREATE INDEX IF NOT EXISTS idx_trading_orders_trigger_pending ON trading_orders (
    trigger_type,
    trigger_status,
    trigger_price
)
WHERE
    trigger_type IS NOT NULL
    AND trigger_status = 'pending';

-- Create index for user's conditional orders
CREATE INDEX IF NOT EXISTS idx_trading_orders_user_conditional ON trading_orders (
    user_id,
    trigger_type,
    trigger_status
)
WHERE
    trigger_type IS NOT NULL;

COMMENT ON COLUMN trading_orders.trigger_price IS 'Price that triggers the conditional order';

COMMENT ON COLUMN trading_orders.trigger_type IS 'Type of conditional order: stop_loss, take_profit, trailing_stop';

COMMENT ON COLUMN trading_orders.trigger_status IS 'Status of the trigger: pending, triggered, cancelled, expired';

COMMENT ON COLUMN trading_orders.triggered_at IS 'Timestamp when the order was triggered';

COMMENT ON COLUMN trading_orders.trailing_offset IS 'Price offset for trailing stop orders';