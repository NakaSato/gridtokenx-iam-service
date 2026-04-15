-- Phase 11: Oracle Bridge Reliability - Aligns meters schema with repository expectations
-- Added: April 02, 2026

-- Add public_key and status columns to meters table
-- These are required by the OracleRepository and for ZK-attestation verification
ALTER TABLE meters 
ADD COLUMN IF NOT EXISTS public_key VARCHAR(255),
ADD COLUMN IF NOT EXISTS status VARCHAR(50) DEFAULT 'active';

-- Add unique index for public key to support signature verification path
CREATE UNIQUE INDEX IF NOT EXISTS idx_meters_public_key 
ON meters(public_key) 
WHERE public_key IS NOT NULL;

-- Add comments for documentation
COMMENT ON COLUMN meters.public_key IS 'Ed25519 public key (base58 encoded) used for IoT device signature verification';
COMMENT ON COLUMN meters.status IS 'Operating status of the meter (active, maintenance, decommissioning)';
