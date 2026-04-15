-- Add SMS support and push tokens to user notification preferences
-- Migration: 20260226000001_add_sms_and_push_tokens

ALTER TABLE user_notification_preferences 
ADD COLUMN IF NOT EXISTS sms_enabled BOOLEAN DEFAULT false,
ADD COLUMN IF NOT EXISTS push_token TEXT;

-- Add phone_number to users if not already present (it should be, but let's be safe)
DO $$ 
BEGIN 
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='users' AND column_name='phone_number') THEN
        ALTER TABLE users ADD COLUMN phone_number VARCHAR(20);
    END IF;
END $$;

COMMENT ON COLUMN user_notification_preferences.sms_enabled IS 'Whether to send SMS notifications';
COMMENT ON COLUMN user_notification_preferences.push_token IS 'Firebase Cloud Messaging token for mobile push';
