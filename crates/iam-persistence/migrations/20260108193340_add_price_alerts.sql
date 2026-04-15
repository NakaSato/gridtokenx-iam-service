-- Add price alerts for user-defined price notifications
-- Migration: 20260108193340_add_price_alerts

-- Create alert condition enum
DO $$ BEGIN
    CREATE TYPE alert_condition AS ENUM ('above', 'below', 'crosses');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Create alert status enum
DO $$ BEGIN
    CREATE TYPE alert_status AS ENUM ('active', 'triggered', 'cancelled');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Price alerts table
CREATE TABLE IF NOT EXISTS price_alerts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

-- Alert configuration
target_price DECIMAL(20, 8) NOT NULL,
condition alert_condition NOT NULL,

-- Status tracking
status alert_status DEFAULT 'active',
triggered_at TIMESTAMPTZ,
triggered_price DECIMAL(20, 8),

-- Options
repeat BOOLEAN DEFAULT false, note VARCHAR(200),

-- Timestamps
created_at TIMESTAMPTZ DEFAULT NOW() );

-- Index for efficient price checking
CREATE INDEX IF NOT EXISTS idx_price_alerts_active ON price_alerts (status, target_price)
WHERE
    status = 'active';

-- Index for user's alerts
CREATE INDEX IF NOT EXISTS idx_price_alerts_user ON price_alerts (user_id, status);

-- Comments
COMMENT ON TABLE price_alerts IS 'User-defined price alerts that trigger notifications';

COMMENT ON COLUMN price_alerts.condition IS 'Trigger when price goes above, below, or crosses the target';

COMMENT ON COLUMN price_alerts.repeat IS 'If true, re-arm the alert after triggering';