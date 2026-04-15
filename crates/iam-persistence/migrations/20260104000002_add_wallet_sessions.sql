-- Session-based wallet security
-- Users unlock wallet once, then can trade for 30 days without password
-- New device login invalidates all previous sessions

CREATE TABLE wallet_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

-- Session identification
session_token VARCHAR(128) NOT NULL UNIQUE,
device_fingerprint VARCHAR(255) NOT NULL,
device_name VARCHAR(100),
ip_address INET,
user_agent TEXT,

-- Encrypted key cache (encrypted with session_token)
cached_key_encrypted BYTEA NOT NULL,
key_salt BYTEA NOT NULL,
key_iv BYTEA NOT NULL,

-- Timestamps
created_at TIMESTAMPTZ DEFAULT NOW (),
expires_at TIMESTAMPTZ NOT NULL,
last_used_at TIMESTAMPTZ DEFAULT NOW (),

-- Status
is_active BOOLEAN DEFAULT true,
    revoked_at TIMESTAMPTZ,
    revoked_reason VARCHAR(50)  -- 'new_device', 'user_logout', 'expired', 'manual'
);

-- Indexes for efficient queries
CREATE INDEX idx_wallet_sessions_user_active ON wallet_sessions (user_id, is_active)
WHERE
    is_active = true;

CREATE INDEX idx_wallet_sessions_token ON wallet_sessions (session_token)
WHERE
    is_active = true;

CREATE INDEX idx_wallet_sessions_expires ON wallet_sessions (expires_at)
WHERE
    is_active = true;

CREATE INDEX idx_wallet_sessions_device ON wallet_sessions (user_id, device_fingerprint);

-- Rate limiting table for unlock attempts
CREATE TABLE wallet_unlock_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    ip_address INET,
    attempted_at TIMESTAMPTZ DEFAULT NOW (),
    success BOOLEAN DEFAULT false
);

CREATE INDEX idx_wallet_unlock_attempts_user ON wallet_unlock_attempts (user_id, attempted_at);

CREATE INDEX idx_wallet_unlock_attempts_ip ON wallet_unlock_attempts (ip_address, attempted_at);

-- Track if user has migrated to user-password encryption
ALTER TABLE users
ADD COLUMN IF NOT EXISTS wallet_encryption_version INTEGER DEFAULT 1;
-- 1 = master secret (legacy)
-- 2 = user password (new)

COMMENT ON COLUMN users.wallet_encryption_version IS '1=master secret encryption (legacy), 2=user password encryption (new)';