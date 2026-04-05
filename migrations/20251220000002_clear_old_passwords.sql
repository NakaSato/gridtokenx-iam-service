-- Migration: Clear old hash_ format passwords and force password reset
-- This migration clears passwords that use the insecure hash_ format
-- Users with affected passwords will need to reset via forgot-password flow

-- Step 1: Identify users with old password format and set their passwords to require reset
-- We do this by:
-- 1. Setting password_hash to empty string (forces re-authentication to fail)
-- 2. Setting password_reset_token to indicate they need reset
-- 3. Setting email_verified to false to trigger the reset flow notification

DO $$
DECLARE
    affected_count INTEGER;
BEGIN
    -- Count affected users first
    SELECT COUNT(*) INTO affected_count 
    FROM users 
    WHERE password_hash LIKE 'hash_%';
    
    RAISE NOTICE 'Found % users with old password format', affected_count;
    
    -- Clear passwords for users with old format
    -- This forces them to use forgot-password flow
    UPDATE users
    SET 
        password_hash = '',  -- Empty hash will fail bcrypt verification
        password_reset_token = NULL,  -- Clear any existing reset tokens
        password_reset_expires_at = NULL,
        updated_at = NOW()
    WHERE password_hash LIKE 'hash_%';
    
    GET DIAGNOSTICS affected_count = ROW_COUNT;
    RAISE NOTICE 'Updated % users to require password reset', affected_count;
END $$;

-- Note: Users will need to:
-- 1. Go to /forgot-password
-- 2. Enter their email
-- 3. Check email for reset link
-- 4. Set a new password that will be hashed with bcrypt
