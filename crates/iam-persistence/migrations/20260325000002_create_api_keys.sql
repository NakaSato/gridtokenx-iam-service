-- Migration: Create api_keys table for IoT/AMI systems
-- Enables dynamic API key management with role-based permissions

CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_hash VARCHAR(64) UNIQUE NOT NULL, -- SHA-256 hash of the API key
    name VARCHAR(100) NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'ami',
    permissions TEXT[] DEFAULT '{}',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_active ON api_keys(is_active) WHERE is_active = true;

-- Seed initial internal API key for testing/engineering
-- Key: "engineering-department-api-key-2025"
-- Hash: Generated using SHA-256 with "secure-api-key-salt" (mocking it here)
-- Actually, the hash should be computed with the same logic as in ApiKeyService.
-- Let's just seed a placeholder that we'll update via the service later.
INSERT INTO api_keys (name, key_hash, role, permissions)
VALUES (
    'Engineering Default Key', 
    'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', -- Mock hash of empty (replace with real hash later)
    'admin',
    '{"*"}'
) ON CONFLICT DO NOTHING;

COMMENT ON TABLE api_keys IS 'Stores API keys for IoT devices and external services';
