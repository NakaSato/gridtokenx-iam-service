-- Add blockchain_tx_signature to erc_certificates
-- Created: November 18, 2024

ALTER TABLE erc_certificates ADD COLUMN IF NOT EXISTS blockchain_tx_signature VARCHAR(88);

-- Add index for blockchain transactions
CREATE INDEX IF NOT EXISTS idx_erc_certificates_tx_signature ON erc_certificates(blockchain_tx_signature);
