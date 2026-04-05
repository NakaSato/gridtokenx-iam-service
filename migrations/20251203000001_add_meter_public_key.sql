-- Add meter public key support for signature verification
-- Migration: 20251203000001_add_meter_public_key.sql

-- Add meter_public_key column to meter_registry
ALTER TABLE meter_registry
ADD COLUMN IF NOT EXISTS meter_public_key VARCHAR(255);

-- Create unique index on meter_public_key
CREATE UNIQUE INDEX IF NOT EXISTS idx_meter_public_key 
ON meter_registry(meter_public_key) 
WHERE meter_public_key IS NOT NULL;

-- Add comment for documentation
COMMENT ON COLUMN meter_registry.meter_public_key IS 'Ed25519 public key (base58 encoded) for signature verification';
