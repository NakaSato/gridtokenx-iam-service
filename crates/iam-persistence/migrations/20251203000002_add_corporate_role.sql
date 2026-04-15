-- Add corporate role to user role constraint
-- Created: December 3, 2025

-- Step 1: Drop existing constraint
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_user_role;

-- Step 2: Add updated constraint with all required roles including corporate
ALTER TABLE users ADD CONSTRAINT chk_user_role 
    CHECK (role IN ('user', 'admin', 'prosumer', 'consumer', 'corporate'));

-- Step 3: Update index to include new roles
DROP INDEX IF EXISTS idx_users_role;
CREATE INDEX IF NOT EXISTS idx_users_role ON users(role) 
    WHERE role IN ('user', 'admin', 'prosumer', 'consumer', 'corporate');

-- Step 4: Add comment explaining role types
COMMENT ON COLUMN users.role IS 'User role: user (default), admin, prosumer, consumer, corporate (PEA/MEA utility)';
