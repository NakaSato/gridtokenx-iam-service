-- Add user wallets table for multi-wallet support
-- Migration: 20260108200337_add_user_wallets

-- User wallets table - allows multiple wallet addresses per user
CREATE TABLE IF NOT EXISTS user_wallets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wallet_address VARCHAR(64) NOT NULL,
    label VARCHAR(50),
    is_primary BOOLEAN DEFAULT false,
    verified BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),

-- Each wallet address can only be linked once globally
CONSTRAINT unique_wallet_address UNIQUE (wallet_address) );

-- Index for user lookups
CREATE INDEX IF NOT EXISTS idx_user_wallets_user ON user_wallets (user_id);

-- Index for wallet address lookups
CREATE INDEX IF NOT EXISTS idx_user_wallets_address ON user_wallets (wallet_address);

-- Ensure only one primary wallet per user
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_wallets_primary ON user_wallets (user_id)
WHERE
    is_primary = true;

-- Migrate existing wallet_address from users table
-- This ensures backwards compatibility
INSERT INTO
    user_wallets (
        user_id,
        wallet_address,
        label,
        is_primary,
        verified,
        created_at
    )
SELECT
    id,
    wallet_address,
    'Primary',
    true,
    true,
    created_at
FROM users
WHERE
    wallet_address IS NOT NULL
ON CONFLICT (wallet_address) DO NOTHING;

-- Comments
COMMENT ON TABLE user_wallets IS 'Links multiple wallet addresses to user accounts';

COMMENT ON COLUMN user_wallets.label IS 'Optional user-defined label for the wallet';

COMMENT ON COLUMN user_wallets.is_primary IS 'Primary wallet used for trading by default';

COMMENT ON COLUMN user_wallets.verified IS 'Whether the wallet ownership has been verified';