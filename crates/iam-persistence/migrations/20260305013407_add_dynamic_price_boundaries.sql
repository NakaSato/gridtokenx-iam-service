-- Admin Portal UI: Dynamic Price Boundaries
-- Migrates the static .env min/max prices into the p2p_config table

INSERT INTO p2p_config (config_key, config_value, config_type, description, category) VALUES
    ('pricing.min_price_per_kwh', 2.20, 'decimal', 'Minimum allowed P2P energy price (THB/kWh)', 'pricing'),
    ('pricing.max_price_per_kwh', 4.15, 'decimal', 'Maximum allowed P2P energy price (THB/kWh)', 'pricing')
ON CONFLICT (config_key) DO NOTHING;
