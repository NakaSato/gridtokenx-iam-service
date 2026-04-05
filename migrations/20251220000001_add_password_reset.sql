-- Password Reset System
-- Created: December 20, 2024

-- Add password reset columns to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_reset_token VARCHAR(128);
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_reset_expires_at TIMESTAMPTZ;

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_users_password_reset_token ON users(password_reset_token) 
    WHERE password_reset_token IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_password_reset_expires ON users(password_reset_expires_at) 
    WHERE password_reset_expires_at IS NOT NULL;
