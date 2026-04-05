-- Add remaining erc_certificates columns
-- Created: November 18, 2024

-- Add issuer_wallet column
ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS issuer_wallet VARCHAR(88);

-- Update tx_signature column name in erc_certificate_transfers
ALTER TABLE erc_certificate_transfers DROP COLUMN IF EXISTS tx_signature;
ALTER TABLE erc_certificate_transfers ADD COLUMN IF NOT EXISTS blockchain_tx_signature VARCHAR(88);
