-- Add blockchain_registered column to users table
-- Created: November 19, 2024

-- Add blockchain_registered column to track if user has completed blockchain registration
ALTER TABLE users ADD COLUMN IF NOT EXISTS blockchain_registered BOOLEAN DEFAULT false;

-- Create index for blockchain_registered column for better query performance
CREATE INDEX IF NOT EXISTS idx_users_blockchain_registered ON users(blockchain_registered);

-- Update existing users: set blockchain_registered to true if they have a wallet_address
UPDATE users SET blockchain_registered = true WHERE wallet_address IS NOT NULL;
