-- Migration: Add blockchain registration status to users table
-- Purpose: Track if a user has been successfully onboarded on the Solana blockchain

-- 1. Add new columns
ALTER TABLE users ADD COLUMN IF NOT EXISTS blockchain_registered BOOLEAN DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS user_account_pda VARCHAR(88);
ALTER TABLE users ADD COLUMN IF NOT EXISTS shard_id SMALLINT;

-- 2. Add indexes for performance
CREATE INDEX IF NOT EXISTS idx_users_blockchain_registered ON users(blockchain_registered) WHERE blockchain_registered = true;
CREATE INDEX IF NOT EXISTS idx_users_pda ON users(user_account_pda) WHERE user_account_pda IS NOT NULL;

-- 3. Comments
COMMENT ON COLUMN users.blockchain_registered IS 'Flag indicating if the user has been registered in the on-chain Registry program';
COMMENT ON COLUMN users.user_account_pda IS 'The derived Solana Program Derived Address (PDA) for the UserAccount state';
COMMENT ON COLUMN users.shard_id IS 'The on-chain registry shard ID assigned to this user';
