-- Migration: Add grid_status_history table
-- Created: January 4, 2026

CREATE TABLE IF NOT EXISTS grid_status_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    total_generation FLOAT8 NOT NULL,
    total_consumption FLOAT8 NOT NULL,
    net_balance FLOAT8 NOT NULL,
    active_meters BIGINT NOT NULL,
    co2_saved_kg FLOAT8 NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW ()
);

CREATE INDEX idx_grid_status_history_timestamp ON grid_status_history (timestamp DESC);

-- Add a comment for documentation
COMMENT ON TABLE grid_status_history IS 'Stores periodic snapshots of aggregate grid metrics for historical analytics.';