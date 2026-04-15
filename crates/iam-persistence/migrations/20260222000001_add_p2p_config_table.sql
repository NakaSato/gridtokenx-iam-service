-- P2P Market Configuration Table
-- Allows admin to dynamically adjust wheeling charges, loss factors, and base prices
-- without deploying new smart contracts or restarting services
-- Created: 2026-02-22

-- 1. Create p2p_config table for market pricing parameters
CREATE TABLE IF NOT EXISTS p2p_config (
    id SERIAL PRIMARY KEY,
    config_key VARCHAR(100) NOT NULL UNIQUE,
    config_value NUMERIC(20, 8) NOT NULL,
    config_type VARCHAR(20) NOT NULL DEFAULT 'decimal', -- 'decimal', 'integer', 'string', 'json'
    description TEXT,
    category VARCHAR(50) NOT NULL DEFAULT 'general', -- 'wheeling', 'loss', 'pricing', 'general'
    is_active BOOLEAN NOT NULL DEFAULT true,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by INTEGER -- No FK constraint to avoid migration issues
);

-- 2. Create index for efficient lookups
CREATE INDEX IF NOT EXISTS idx_p2p_config_key ON p2p_config(config_key);
CREATE INDEX IF NOT EXISTS idx_p2p_config_category ON p2p_config(category);
CREATE INDEX IF NOT EXISTS idx_p2p_config_active ON p2p_config(is_active);

-- 3. Insert default wheeling charge configuration
INSERT INTO p2p_config (config_key, config_value, config_type, description, category) VALUES
    ('wheeling.same_zone', 0.50, 'decimal', 'Wheeling charge for same-zone transactions (THB/kWh)', 'wheeling'),
    ('wheeling.adjacent_zone', 1.00, 'decimal', 'Wheeling charge for adjacent zones (THB/kWh)', 'wheeling'),
    ('wheeling.base_charge', 1.50, 'decimal', 'Base wheeling charge for cross-zone transactions (THB/kWh)', 'wheeling'),
    ('wheeling.distance_rate', 0.10, 'decimal', 'Additional charge per zone distance (THB/kWh)', 'wheeling'),
    ('wheeling.fallback_rate', 0.02, 'decimal', 'Fallback wheeling rate base (API only)', 'wheeling'),
    ('wheeling.fallback_distance_rate', 0.015, 'decimal', 'Fallback wheeling rate per km (API only)', 'wheeling')
ON CONFLICT (config_key) DO NOTHING;

-- 4. Insert default loss factor configuration
INSERT INTO p2p_config (config_key, config_value, config_type, description, category) VALUES
    ('loss.same_zone', 0.01, 'decimal', 'Technical loss factor for same-zone (1% = 0.01)', 'loss'),
    ('loss.adjacent_zone', 0.03, 'decimal', 'Technical loss factor for adjacent zones (3% = 0.03)', 'loss'),
    ('loss.base_loss', 0.03, 'decimal', 'Base loss factor for cross-zone', 'loss'),
    ('loss.distance_rate', 0.01, 'decimal', 'Additional loss per zone distance', 'loss'),
    ('loss.max_loss', 0.15, 'decimal', 'Maximum allowed loss factor (15% = 0.15)', 'loss'),
    ('loss.fallback_rate', 0.01, 'decimal', 'Fallback loss factor base (API only)', 'loss'),
    ('loss.fallback_distance_rate', 0.005, 'decimal', 'Fallback loss factor per km (API only)', 'loss')
ON CONFLICT (config_key) DO NOTHING;

-- 5. Insert default market pricing configuration
INSERT INTO p2p_config (config_key, config_value, config_type, description, category) VALUES
    ('pricing.base_price_thb_kwh', 4.00, 'decimal', 'Base P2P energy price (THB/kWh)', 'pricing'),
    ('pricing.grid_import_price_thb_kwh', 4.50, 'decimal', 'Price when buying from main grid (THB/kWh)', 'pricing'),
    ('pricing.grid_export_price_thb_kwh', 2.20, 'decimal', 'FiT rate when selling to grid (THB/kWh)', 'pricing'),
    ('pricing.transaction_fee_bps', 25.00, 'decimal', 'Transaction fee in basis points (0.25% = 25 bps)', 'pricing'),
    ('pricing.price_sensitivity_alpha', 0.20, 'decimal', 'Dynamic pricing sensitivity coefficient', 'pricing'),
    ('pricing.max_price_deviation', 0.20, 'decimal', 'Max price deviation from base (20% = 0.20)', 'pricing')
ON CONFLICT (config_key) DO NOTHING;

-- 6. Insert zone-specific wheeling charges (intra_zone, inter_zone)
INSERT INTO p2p_config (config_key, config_value, config_type, description, category) VALUES
    ('wheeling.zone_0', 0.0, 'decimal', 'Wheeling charge for zone distance 0 (same zone)', 'wheeling'),
    ('wheeling.zone_1', 0.02, 'decimal', 'Wheeling charge for zone distance 1', 'wheeling'),
    ('wheeling.zone_2', 0.035, 'decimal', 'Wheeling charge for zone distance 2', 'wheeling'),
    ('wheeling.zone_3', 0.05, 'decimal', 'Wheeling charge for zone distance 3', 'wheeling'),
    ('loss.zone_0', 1.0, 'decimal', 'Loss factor for zone distance 0', 'loss'),
    ('loss.zone_1', 1.01, 'decimal', 'Loss factor for zone distance 1 (1% loss)', 'loss'),
    ('loss.zone_2', 1.025, 'decimal', 'Loss factor for zone distance 2 (2.5% loss)', 'loss'),
    ('loss.zone_3', 1.04, 'decimal', 'Loss factor for zone distance 3 (4% loss)', 'loss')
ON CONFLICT (config_key) DO NOTHING;

-- 7. Add audit trigger for config changes (optional - logs all updates)
CREATE TABLE IF NOT EXISTS p2p_config_audit (
    id SERIAL PRIMARY KEY,
    config_key VARCHAR(100) NOT NULL,
    old_value NUMERIC(20, 8),
    new_value NUMERIC(20, 8),
    changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    changed_by INTEGER, -- No FK constraint to avoid migration issues
    change_reason TEXT
);

CREATE INDEX IF NOT EXISTS idx_p2p_config_audit_key ON p2p_config_audit(config_key);
CREATE INDEX IF NOT EXISTS idx_p2p_config_audit_time ON p2p_config_audit(changed_at);

-- 8. Create function to auto-log config changes
CREATE OR REPLACE FUNCTION log_p2p_config_change()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.config_value IS DISTINCT FROM NEW.config_value THEN
        INSERT INTO p2p_config_audit (config_key, old_value, new_value, changed_by)
        VALUES (OLD.config_key, OLD.config_value, NEW.config_value, NEW.updated_by);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 9. Attach trigger to p2p_config table
DROP TRIGGER IF EXISTS p2p_config_audit_trigger ON p2p_config;
CREATE TRIGGER p2p_config_audit_trigger
    AFTER UPDATE ON p2p_config
    FOR EACH ROW
    EXECUTE FUNCTION log_p2p_config_change();

-- 10. Add comments
COMMENT ON TABLE p2p_config IS 'Dynamic P2P market configuration - editable by admin without contract redeployment';
COMMENT ON TABLE p2p_config_audit IS 'Audit trail for all P2P config changes';
