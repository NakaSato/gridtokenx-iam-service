-- Add carbon credits tracking tables
-- Migration: 20260108204628_add_carbon_credits

-- Carbon credit status enum
DO $$ BEGIN
    CREATE TYPE carbon_status AS ENUM ('active', 'retired', 'transferred');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Transaction status enum
DO $$ BEGIN
    CREATE TYPE carbon_transaction_status AS ENUM ('pending', 'completed', 'failed', 'cancelled');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Carbon credits earned from renewable energy
CREATE TABLE IF NOT EXISTS carbon_credits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

-- Credit details
amount DECIMAL(20, 8) NOT NULL, -- Credits in tons CO2
source VARCHAR(50) NOT NULL, -- 'solar', 'wind', 'meter_production', 'purchase'
source_reference_id UUID, -- Optional reference to meter reading, order, etc.

-- Status
status carbon_status DEFAULT 'active',

-- Metadata
description VARCHAR(200), created_at TIMESTAMPTZ DEFAULT NOW() );

-- Carbon credit transactions (transfers between users)
CREATE TABLE IF NOT EXISTS carbon_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

-- Parties
from_user_id UUID REFERENCES users (id), -- NULL for purchases from platform
to_user_id UUID NOT NULL REFERENCES users (id),

-- Transaction details
amount DECIMAL(20, 8) NOT NULL,
price_per_credit DECIMAL(20, 8), -- Price if sold/purchased
total_value DECIMAL(20, 8),

-- Status
status carbon_transaction_status DEFAULT 'completed',

-- Metadata
notes VARCHAR(200), created_at TIMESTAMPTZ DEFAULT NOW() );

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_carbon_credits_user ON carbon_credits (user_id, status);

CREATE INDEX IF NOT EXISTS idx_carbon_credits_source ON carbon_credits (source, created_at);

CREATE INDEX IF NOT EXISTS idx_carbon_transactions_to ON carbon_transactions (to_user_id, created_at);

CREATE INDEX IF NOT EXISTS idx_carbon_transactions_from ON carbon_transactions (from_user_id, created_at);

-- Comments
COMMENT ON TABLE carbon_credits IS 'Carbon credits earned from renewable energy production';

COMMENT ON TABLE carbon_transactions IS 'Carbon credit transfers between users';

COMMENT ON COLUMN carbon_credits.amount IS 'Amount in metric tons of CO2 equivalent';

COMMENT ON COLUMN carbon_credits.source IS 'Source of credits: solar, wind, meter_production, purchase';