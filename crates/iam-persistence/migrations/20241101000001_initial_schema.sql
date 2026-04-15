-- Initial schema for GridTokenX P2P Energy Trading Platform
-- Created: November 1, 2024

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- =========================================================================
-- USERS TABLE
-- =========================================================================
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    username VARCHAR(50) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    wallet_address VARCHAR(88) UNIQUE,
    role VARCHAR(20) NOT NULL DEFAULT 'user',
    user_type VARCHAR(20),
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    is_active BOOLEAN DEFAULT true,
    registered_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_wallet ON users(wallet_address);
CREATE INDEX idx_users_role ON users(role);
CREATE INDEX idx_users_is_active ON users(is_active);

-- =========================================================================
-- MARKET EPOCHS TABLE (15-minute trading windows)
-- =========================================================================
CREATE TABLE market_epochs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    epoch_number BIGINT UNIQUE NOT NULL,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    status VARCHAR(20) NOT NULL,
    clearing_price NUMERIC(20, 8),
    total_volume NUMERIC(20, 8) DEFAULT 0,
    total_orders BIGINT DEFAULT 0,
    matched_orders BIGINT DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_epoch_status CHECK (status IN ('pending', 'active', 'cleared', 'settled'))
);

CREATE INDEX idx_market_epochs_time ON market_epochs(start_time, end_time);
CREATE INDEX idx_market_epochs_status ON market_epochs(status);
CREATE INDEX idx_market_epochs_number ON market_epochs(epoch_number);

-- =========================================================================
-- TRADING ORDERS TABLE
-- =========================================================================
CREATE TABLE trading_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    epoch_id UUID REFERENCES market_epochs(id) ON DELETE SET NULL,
    order_type VARCHAR(10) NOT NULL,
    energy_amount NUMERIC(20, 8) NOT NULL,
    price_per_kwh NUMERIC(20, 8) NOT NULL,
    filled_amount NUMERIC(20, 8) DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    transaction_hash VARCHAR(66),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    settled_at TIMESTAMPTZ,
    CONSTRAINT chk_order_type CHECK (order_type IN ('buy', 'sell')),
    CONSTRAINT chk_order_status CHECK (status IN ('pending', 'active', 'partially_filled', 'filled', 'settled', 'cancelled')),
    CONSTRAINT chk_energy_amount CHECK (energy_amount > 0),
    CONSTRAINT chk_price CHECK (price_per_kwh > 0)
);

CREATE INDEX idx_trading_orders_user ON trading_orders(user_id);
CREATE INDEX idx_trading_orders_epoch ON trading_orders(epoch_id);
CREATE INDEX idx_trading_orders_status ON trading_orders(status);
CREATE INDEX idx_trading_orders_type ON trading_orders(order_type);
CREATE INDEX idx_trading_orders_created ON trading_orders(created_at);

-- =========================================================================
-- ORDER MATCHES TABLE
-- =========================================================================
CREATE TABLE order_matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    epoch_id UUID NOT NULL REFERENCES market_epochs(id) ON DELETE CASCADE,
    buy_order_id UUID NOT NULL REFERENCES trading_orders(id) ON DELETE CASCADE,
    sell_order_id UUID NOT NULL REFERENCES trading_orders(id) ON DELETE CASCADE,
    matched_amount NUMERIC(20, 8) NOT NULL,
    match_price NUMERIC(20, 8) NOT NULL,
    match_time TIMESTAMPTZ DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    settlement_id UUID,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_match_status CHECK (status IN ('pending', 'settled', 'failed')),
    CONSTRAINT chk_matched_amount CHECK (matched_amount > 0)
);

CREATE INDEX idx_order_matches_epoch ON order_matches(epoch_id);
CREATE INDEX idx_order_matches_buy_order ON order_matches(buy_order_id);
CREATE INDEX idx_order_matches_sell_order ON order_matches(sell_order_id);
CREATE INDEX idx_order_matches_status ON order_matches(status);

-- =========================================================================
-- SETTLEMENTS TABLE
-- =========================================================================
CREATE TABLE settlements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    epoch_id UUID NOT NULL REFERENCES market_epochs(id) ON DELETE CASCADE,
    buyer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    seller_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    energy_amount NUMERIC(20, 8) NOT NULL,
    price_per_kwh NUMERIC(20, 8) NOT NULL,
    total_amount NUMERIC(20, 8) NOT NULL,
    fee_amount NUMERIC(20, 8) NOT NULL DEFAULT 0,
    net_amount NUMERIC(20, 8) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    transaction_hash VARCHAR(66),
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_settlement_status CHECK (status IN ('pending', 'processing', 'completed', 'failed'))
);

CREATE INDEX idx_settlements_epoch ON settlements(epoch_id);
CREATE INDEX idx_settlements_buyer ON settlements(buyer_id);
CREATE INDEX idx_settlements_seller ON settlements(seller_id);
CREATE INDEX idx_settlements_status ON settlements(status);
CREATE INDEX idx_settlements_transaction ON settlements(transaction_hash);

-- Add foreign key to order_matches after settlements table is created
ALTER TABLE order_matches ADD CONSTRAINT fk_order_matches_settlement 
    FOREIGN KEY (settlement_id) REFERENCES settlements(id) ON DELETE SET NULL;

-- =========================================================================
-- METER READINGS TABLE
-- =========================================================================
CREATE TABLE meter_readings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    meter_id VARCHAR(50) NOT NULL,
    wallet_address VARCHAR(88) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    energy_generated NUMERIC(12, 4),
    energy_consumed NUMERIC(12, 4),
    surplus_energy NUMERIC(12, 4),
    deficit_energy NUMERIC(12, 4),
    battery_level NUMERIC(5, 2),
    temperature NUMERIC(5, 2),
    voltage NUMERIC(8, 2),
    current NUMERIC(8, 2),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_meter_readings_meter ON meter_readings(meter_id);
CREATE INDEX idx_meter_readings_wallet ON meter_readings(wallet_address);
CREATE INDEX idx_meter_readings_timestamp ON meter_readings(timestamp);

-- =========================================================================
-- ENERGY CERTIFICATES TABLE (RECs/ERCs)
-- =========================================================================
CREATE TABLE energy_certificates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    certificate_id VARCHAR(100) UNIQUE NOT NULL,
    wallet_address VARCHAR(88) NOT NULL,
    energy_amount NUMERIC(20, 8) NOT NULL,
    certificate_type VARCHAR(20) NOT NULL,
    issuance_date TIMESTAMPTZ NOT NULL,
    expiry_date TIMESTAMPTZ,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_cert_type CHECK (certificate_type IN ('REC', 'ERC', 'IREC')),
    CONSTRAINT chk_cert_status CHECK (status IN ('active', 'retired', 'expired', 'transferred'))
);

CREATE INDEX idx_certificates_wallet ON energy_certificates(wallet_address);
CREATE INDEX idx_certificates_type ON energy_certificates(certificate_type);
CREATE INDEX idx_certificates_status ON energy_certificates(status);

-- =========================================================================
-- USER ACTIVITIES TABLE (Audit Log)
-- =========================================================================
CREATE TABLE user_activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    activity_type VARCHAR(50) NOT NULL,
    description TEXT,
    ip_address INET,
    user_agent TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_user_activities_user ON user_activities(user_id);
CREATE INDEX idx_user_activities_type ON user_activities(activity_type);
CREATE INDEX idx_user_activities_created ON user_activities(created_at);

-- =========================================================================
-- UPDATED_AT TRIGGERS
-- =========================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_market_epochs_updated_at BEFORE UPDATE ON market_epochs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_trading_orders_updated_at BEFORE UPDATE ON trading_orders
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_order_matches_updated_at BEFORE UPDATE ON order_matches
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_settlements_updated_at BEFORE UPDATE ON settlements
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_energy_certificates_updated_at BEFORE UPDATE ON energy_certificates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
