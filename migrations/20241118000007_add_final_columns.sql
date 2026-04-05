-- Add remaining missing columns
-- Created: November 18, 2024

-- Add issue_date to erc_certificates (rename from issuance_date)
ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS issue_date TIMESTAMPTZ;
UPDATE erc_certificates SET issue_date = issuance_date WHERE issue_date IS NULL;

-- Add submitted_at to meter_readings  
ALTER TABLE meter_readings ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ DEFAULT NOW();

-- Fix erc_certificate_transfers columns
ALTER TABLE erc_certificate_transfers DROP COLUMN IF EXISTS from_wallet;
ALTER TABLE erc_certificate_transfers DROP COLUMN IF EXISTS to_wallet;
ALTER TABLE erc_certificate_transfers DROP COLUMN IF EXISTS tx_signature;
ALTER TABLE erc_certificate_transfers ADD COLUMN IF NOT EXISTS from_wallet VARCHAR(88);
ALTER TABLE erc_certificate_transfers ADD COLUMN IF NOT EXISTS to_wallet VARCHAR(88) NOT NULL;
ALTER TABLE erc_certificate_transfers ADD COLUMN IF NOT EXISTS tx_signature VARCHAR(88);
