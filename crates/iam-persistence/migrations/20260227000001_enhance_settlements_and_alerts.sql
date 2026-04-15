-- Migration: Enhance settlements and price_alerts
-- Created: 2026-02-27

-- 1. Update price_alerts table
ALTER TABLE price_alerts 
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

-- Add trigger for updated_at on price_alerts
CREATE TRIGGER update_price_alerts_updated_at BEFORE UPDATE ON price_alerts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- 2. Update settlements table
ALTER TABLE settlements 
ADD COLUMN IF NOT EXISTS error_message TEXT;

-- Comment for clarity
COMMENT ON COLUMN settlements.error_message IS 'Detailed error message from failed settlement attempts';
COMMENT ON COLUMN price_alerts.updated_at IS 'Timestamp of the last modification to the alert';
