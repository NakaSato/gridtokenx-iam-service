-- Email Verification System
-- Created: November 2, 2024

-- Add email verification columns to users table
ALTER TABLE users ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN email_verification_token VARCHAR(128);
ALTER TABLE users ADD COLUMN email_verification_sent_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN email_verification_expires_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;

-- Create indexes for performance
CREATE INDEX idx_users_email_verified ON users(email_verified);
CREATE INDEX idx_users_verification_token ON users(email_verification_token) 
    WHERE email_verification_token IS NOT NULL;
CREATE INDEX idx_users_verification_expires ON users(email_verification_expires_at) 
    WHERE email_verification_expires_at IS NOT NULL;
