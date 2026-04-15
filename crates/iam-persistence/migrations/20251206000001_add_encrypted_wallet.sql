-- Add columns for encrypted wallet storage
ALTER TABLE users 
ADD COLUMN IF NOT EXISTS encrypted_private_key TEXT,
ADD COLUMN IF NOT EXISTS wallet_salt TEXT,
ADD COLUMN IF NOT EXISTS encryption_iv TEXT;

-- Create index for faster lookups if needed (though mostly accessed by ID)
-- CREATE INDEX idx_users_encrypted_key ON users(encrypted_private_key) WHERE encrypted_private_key IS NOT NULL;
