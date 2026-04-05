-- Migration: Add ERC transfer tracking to settlements
-- Created: 2026-02-20

ALTER TABLE settlements 
ADD COLUMN erc_certificate_id VARCHAR(255),
ADD COLUMN erc_transfer_tx VARCHAR(255);

COMMENT ON COLUMN settlements.erc_certificate_id IS 'ID of the ERC certificate transferred in this settlement';
COMMENT ON COLUMN settlements.erc_transfer_tx IS 'Solana transaction signature for the ERC transfer';
