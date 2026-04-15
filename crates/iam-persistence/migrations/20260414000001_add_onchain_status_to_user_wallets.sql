-- Migration: Add blockchain registration status to user_wallets table
-- Purpose: Track if secondary wallets have been successfully registered in the Solana Registry program

-- 1. Add new columns
ALTER TABLE user_wallets ADD COLUMN IF NOT EXISTS blockchain_registered BOOLEAN DEFAULT false;
ALTER TABLE user_wallets ADD COLUMN IF NOT EXISTS user_account_pda VARCHAR(88);
ALTER TABLE user_wallets ADD COLUMN IF NOT EXISTS shard_id SMALLINT;
ALTER TABLE user_wallets ADD COLUMN IF NOT EXISTS blockchain_tx_signature VARCHAR(128);

-- 2. Add indexes for performance
CREATE INDEX IF NOT EXISTS idx_user_wallets_blockchain_registered ON user_wallets(blockchain_registered) WHERE blockchain_registered = true;
CREATE INDEX IF NOT EXISTS idx_user_wallets_pda ON user_wallets(user_account_pda) WHERE user_account_pda IS NOT NULL;

-- 3. Comments
COMMENT ON COLUMN user_wallets.blockchain_registered IS 'Flag indicating if this wallet has been registered in the on-chain Registry program';
COMMENT ON COLUMN user_wallets.user_account_pda IS 'The derived Solana Program Derived Address (PDA) for the UserAccount state for this wallet';
COMMENT ON COLUMN user_wallets.shard_id IS 'The on-chain registry shard ID assigned to this wallet';
COMMENT ON COLUMN user_wallets.blockchain_tx_signature IS 'The transaction signature from the on-chain registration';
