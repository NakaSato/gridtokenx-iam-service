-- Migration: Add onboarding location to users table
-- Purpose: Store the location and type used during initial onboarding for auto-registration of secondary wallets

ALTER TABLE users ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION;
ALTER TABLE users ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION;

COMMENT ON COLUMN users.latitude IS 'Latitude used during on-chain onboarding (e7 format or float)';
COMMENT ON COLUMN users.longitude IS 'Longitude used during on-chain onboarding (e7 format or float)';
