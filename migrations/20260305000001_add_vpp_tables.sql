-- Phase 10: VPP Orchestration & Real-time Balancing
-- Create tables for Virtual Power Plant (VPP) cluster management and dispatch tracking

CREATE TABLE IF NOT EXISTS vpp_clusters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id TEXT NOT NULL UNIQUE,
    zone_id INT,
    total_capacity_kwh FLOAT8 NOT NULL DEFAULT 0.0,
    current_stored_kwh FLOAT8 NOT NULL DEFAULT 0.0,
    soc_percentage FLOAT8 NOT NULL DEFAULT 0.0,
    flex_up_kw FLOAT8 NOT NULL DEFAULT 0.0,
    flex_down_kw FLOAT8 NOT NULL DEFAULT 0.0,
    health_score FLOAT8 NOT NULL DEFAULT 100.0,
    resource_count INT NOT NULL DEFAULT 0,
    last_update TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS vpp_dispatch_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cluster_id TEXT NOT NULL REFERENCES vpp_clusters(cluster_id),
    amount_kw FLOAT8 NOT NULL,
    status TEXT NOT NULL, -- 'Pending', 'In Progress', 'Completed', 'Failed'
    pda_address TEXT,      -- For future blockchain verification
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Seed some initial clusters for testing (linked to existing zones)
INSERT INTO vpp_clusters (cluster_id, zone_id, total_capacity_kwh, current_stored_kwh, soc_percentage, flex_up_kw, flex_down_kw, resource_count, health_score)
VALUES 
('ZONE-A-ST', 1, 500.0, 312.0, 62.4, 120.0, 80.0, 15, 98.5),
('ZONE-B-MT', 2, 250.0, 45.0, 18.0, 45.0, 12.0, 8, 92.0),
('ZONE-C-HP', 3, 1200.0, 890.0, 74.2, 350.0, 210.0, 32, 99.1)
ON CONFLICT (cluster_id) DO NOTHING;

-- Index for cluster lookup
CREATE INDEX IF NOT EXISTS idx_vpp_clusters_zone ON vpp_clusters(zone_id);
CREATE INDEX IF NOT EXISTS idx_vpp_dispatch_timestamp ON vpp_dispatch_history(timestamp);
