-- Add meter verification tables and update meter_readings table
-- This migration adds meter registry functionality to prevent fraudulent readings

-- Create meter_registry table
CREATE TABLE meter_registry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    meter_serial VARCHAR(255) UNIQUE NOT NULL,
    meter_key_hash VARCHAR(255) NOT NULL,
    verification_method VARCHAR(50) NOT NULL DEFAULT 'serial',
    verification_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    manufacturer VARCHAR(255),
    meter_type VARCHAR(50),
    location_address TEXT,
    installation_date DATE,
    verified_at TIMESTAMP WITH TIME ZONE,
    verified_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    verification_proof TEXT,
    metadata JSONB DEFAULT '{}'::jsonb
);

-- Create meter_verification_attempts table
CREATE TABLE meter_verification_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    meter_serial VARCHAR(255) NOT NULL,
    verification_method VARCHAR(50) NOT NULL,
    attempt_status VARCHAR(20) NOT NULL,
    attempt_result VARCHAR(20),
    ip_address INET,
    user_agent TEXT,
    attempted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    failure_reason TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Rename existing meter_id to meter_serial for clarity
ALTER TABLE meter_readings 
RENAME COLUMN meter_id TO meter_serial;

-- Add new meter_id column as UUID reference to meter_registry
ALTER TABLE meter_readings 
ADD COLUMN IF NOT EXISTS meter_id UUID REFERENCES meter_registry(id) ON DELETE SET NULL;

-- Add verification_status column to meter_readings table
ALTER TABLE meter_readings 
ADD COLUMN IF NOT EXISTS verification_status VARCHAR(20) DEFAULT 'legacy_unverified';

-- Create indexes for better performance
CREATE INDEX idx_meter_registry_user_id ON meter_registry(user_id);
CREATE INDEX idx_meter_registry_serial ON meter_registry(meter_serial);
CREATE INDEX idx_meter_registry_status ON meter_registry(verification_status);
CREATE INDEX idx_meter_verification_attempts_user_id ON meter_verification_attempts(user_id);
CREATE INDEX idx_meter_verification_attempts_attempted_at ON meter_verification_attempts(attempted_at);
