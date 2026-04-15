-- Create meters table to match codebase expectations
-- Created: December 21, 2025

CREATE TABLE IF NOT EXISTS meters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    serial_number VARCHAR(100) UNIQUE NOT NULL,
    meter_type VARCHAR(50),
    location TEXT,
    is_verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_meters_user_id ON meters(user_id);
CREATE INDEX IF NOT EXISTS idx_meters_serial_number ON meters(serial_number);

-- Reuse existing trigger function if available
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_meters_updated_at') THEN
        CREATE TRIGGER update_meters_updated_at BEFORE UPDATE ON meters
            FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;
