-- Fix schema mismatches from code expectations
-- Created: November 18, 2024

-- Add missing columns to meter_readings
ALTER TABLE meter_readings ADD COLUMN IF NOT EXISTS reading_timestamp TIMESTAMPTZ DEFAULT NOW();
ALTER TABLE meter_readings ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- Update existing rows to use timestamp column value
UPDATE meter_readings SET reading_timestamp = timestamp WHERE reading_timestamp IS NULL;

-- Add missing columns to erc_certificates
ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS kwh_amount NUMERIC(20, 8);

-- Populate kwh_amount from energy_amount
UPDATE erc_certificates SET kwh_amount = energy_amount WHERE kwh_amount IS NULL;

-- Create erc_certificate_transfers table
CREATE TABLE IF NOT EXISTS erc_certificate_transfers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    certificate_id UUID NOT NULL REFERENCES erc_certificates(id) ON DELETE CASCADE,
    from_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    to_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    transfer_date TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    transaction_hash VARCHAR(88),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_erc_transfers_certificate ON erc_certificate_transfers(certificate_id);
CREATE INDEX IF NOT EXISTS idx_erc_transfers_from_user ON erc_certificate_transfers(from_user_id);
CREATE INDEX IF NOT EXISTS idx_erc_transfers_to_user ON erc_certificate_transfers(to_user_id);

-- Create indexes for new columns
CREATE INDEX IF NOT EXISTS idx_meter_readings_reading_timestamp ON meter_readings(reading_timestamp);

-- Add trigger for meter_readings updated_at
CREATE TRIGGER update_meter_readings_updated_at BEFORE UPDATE ON meter_readings
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
