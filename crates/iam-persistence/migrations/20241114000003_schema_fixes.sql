-- Schema fixes and additions
-- Created: November 14, 2024

-- Rename energy_certificates to erc_certificates
ALTER TABLE energy_certificates RENAME TO erc_certificates;

-- Add missing columns to erc_certificates
ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS user_id UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS energy_source VARCHAR(50);
ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS vintage_year INTEGER;

-- Add missing columns to meter_readings
ALTER TABLE meter_readings ADD COLUMN IF NOT EXISTS user_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- Add missing columns to trading_orders
ALTER TABLE trading_orders ADD COLUMN IF NOT EXISTS side VARCHAR(10);
ALTER TABLE trading_orders ADD COLUMN IF NOT EXISTS kwh_amount NUMERIC(20, 8);

-- Update side column based on order_type
UPDATE trading_orders SET side = order_type WHERE side IS NULL;

-- Update kwh_amount based on energy_amount
UPDATE trading_orders SET kwh_amount = energy_amount WHERE kwh_amount IS NULL;

-- Create custom enum type for order status (if not exists)
DO $$ BEGIN
    CREATE TYPE order_status AS ENUM ('pending', 'active', 'partially_filled', 'filled', 'settled', 'cancelled');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Create indexes for new columns
CREATE INDEX IF NOT EXISTS idx_erc_certificates_user ON erc_certificates(user_id);
CREATE INDEX IF NOT EXISTS idx_meter_readings_user ON meter_readings(user_id);
CREATE INDEX IF NOT EXISTS idx_trading_orders_side ON trading_orders(side);
