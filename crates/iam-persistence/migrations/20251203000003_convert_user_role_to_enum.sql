-- Create user_role enum
DO $$ BEGIN
    CREATE TYPE user_role AS ENUM ('user', 'admin', 'prosumer', 'consumer', 'corporate');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Drop check constraint
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_user_role;

-- Drop index
DROP INDEX IF EXISTS idx_users_role;

-- Drop default
ALTER TABLE users ALTER COLUMN role DROP DEFAULT;

-- Convert column
ALTER TABLE users 
    ALTER COLUMN role TYPE user_role 
    USING role::user_role;

-- Set default
ALTER TABLE users ALTER COLUMN role SET DEFAULT 'user'::user_role;

-- Recreate index
CREATE INDEX idx_users_role ON users(role);
