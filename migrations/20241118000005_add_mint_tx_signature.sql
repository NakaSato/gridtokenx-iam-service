-- Add remaining missing columns
-- Created: November 18, 2024

-- Add missing column to meter_readings
ALTER TABLE meter_readings ADD COLUMN IF NOT EXISTS mint_tx_signature VARCHAR(88);

-- Add index for mint_tx_signature
CREATE INDEX IF NOT EXISTS idx_meter_readings_mint_tx ON meter_readings(mint_tx_signature);
