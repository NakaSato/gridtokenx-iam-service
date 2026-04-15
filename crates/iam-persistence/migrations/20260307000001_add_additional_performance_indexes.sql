-- Migration: Add Additional Performance Indexes
-- Created: March 7, 2026
-- Description: Add optimized indexes for order book and zone-based queries

-- =========================================================================
-- TRADING ORDERS TABLE - Order Book Optimization
-- =========================================================================

-- Composite index for order book queries (epoch + side + status + price sorting)
-- Critical for the matching engine get_order_book() queries
CREATE INDEX IF NOT EXISTS idx_trading_orders_epoch_side_status_price 
ON trading_orders(epoch_id, side, status, price_per_kwh DESC, created_at ASC)
WHERE status IN ('pending', 'active', 'partially_filled') AND price_per_kwh IS NOT NULL;

-- Partial index for zone-based order queries
CREATE INDEX IF NOT EXISTS idx_trading_orders_zone_active 
ON trading_orders(zone_id, status, price_per_kwh)
WHERE status IN ('pending', 'active', 'partially_filled');

-- Covering index for order lookups by ID with status
CREATE INDEX IF NOT EXISTS idx_trading_orders_id_status 
ON trading_orders(id, status, side, energy_amount, filled_amount, price_per_kwh);

-- =========================================================================
-- ORDER MATCHES TABLE - Analytics & Zone Optimization
-- =========================================================================

-- BRIN index for match_time (efficient for time-series match data)
DROP INDEX IF EXISTS idx_order_matches_match_time;
CREATE INDEX IF NOT EXISTS idx_order_matches_match_time 
ON order_matches USING BRIN(match_time);

-- Composite index for zone + time analytics
CREATE INDEX IF NOT EXISTS idx_order_matches_zone_time 
ON order_matches(zone_id, match_time DESC)
WHERE zone_id IS NOT NULL;

-- Covering index for settlement queries
CREATE INDEX IF NOT EXISTS idx_order_matches_settlement_covering 
ON order_matches(epoch_id, status, buy_order_id, sell_order_id, matched_amount, match_price)
WHERE status = 'pending';

-- =========================================================================
-- SETTLEMENTS TABLE - User History Optimization
-- =========================================================================

-- Separate indexes for buyer and seller lookups (more efficient than OR condition)
CREATE INDEX IF NOT EXISTS idx_settlements_buyer_time 
ON settlements(buyer_id, created_at DESC)
INCLUDE (energy_amount, price_per_kwh, total_amount, status);

CREATE INDEX IF NOT EXISTS idx_settlements_seller_time 
ON settlements(seller_id, created_at DESC)
INCLUDE (energy_amount, price_per_kwh, total_amount, status);

-- Zone-based settlement queries
CREATE INDEX IF NOT EXISTS idx_settlements_buyer_zone 
ON settlements(buyer_zone_id, created_at DESC)
WHERE buyer_zone_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_settlements_seller_zone 
ON settlements(seller_zone_id, created_at DESC)
WHERE seller_zone_id IS NOT NULL;

-- =========================================================================
-- METER READINGS TABLE - Serial Number Optimization
-- =========================================================================

-- Index for meter serial lookups (frequent in readings handler)
CREATE INDEX IF NOT EXISTS idx_meter_readings_serial_timestamp 
ON meter_readings(meter_serial, timestamp DESC);

-- =========================================================================
-- METERS TABLE - Zone-Based Queries
-- =========================================================================

-- Composite index for zone-based meter lookups
CREATE INDEX IF NOT EXISTS idx_meters_zone_serial 
ON meters(zone_id, serial_number)
WHERE zone_id IS NOT NULL;

-- Index for active meters by serial
CREATE INDEX IF NOT EXISTS idx_meters_serial_active 
ON meters(serial_number, zone_id);

-- =========================================================================
-- USERS TABLE - Balance/Escrow Queries
-- =========================================================================

-- Index for wallet address lookups with balance info
CREATE INDEX IF NOT EXISTS idx_users_wallet_balance 
ON users(wallet_address, balance, locked_amount)
WHERE wallet_address IS NOT NULL;

-- =========================================================================
-- EPOCH TABLE - Status Queries
-- =========================================================================

-- Composite index for epoch status + time queries
CREATE INDEX IF NOT EXISTS idx_market_epochs_status_end 
ON market_epochs(status, end_time DESC, start_time DESC);

-- Partial index for active epochs
CREATE INDEX IF NOT EXISTS idx_market_epochs_active 
ON market_epochs(start_time DESC, end_time DESC)
WHERE status IN ('active', 'pending');

-- =========================================================================
-- ESCROW RECORDS TABLE - Order Lookup Optimization
-- =========================================================================

-- Index for order-based escrow lookups
CREATE INDEX IF NOT EXISTS idx_escrow_records_order_status 
ON escrow_records(order_id, status)
WHERE status = 'locked';

-- =========================================================================
-- V_INDEX_USAGE VIEW UPDATE
-- =========================================================================

-- Recreate the index usage monitoring view
CREATE OR REPLACE VIEW v_index_usage AS
SELECT
    schemaname,
    relname as tablename,
    indexrelname as indexname,
    idx_scan as scans,
    idx_tup_read as tuples_read,
    idx_tup_fetch as tuples_fetched,
    pg_size_pretty(pg_relation_size(indexrelid)) as index_size,
    CASE 
        WHEN idx_scan > 0 THEN pg_size_pretty(pg_relation_size(indexrelid)::bigint / idx_scan)
        ELSE '0 bytes'
    END as bytes_per_scan
FROM pg_stat_user_indexes
WHERE schemaname = 'public'
ORDER BY idx_scan DESC;

-- =========================================================================
-- ANALYSIS QUERIES
-- =========================================================================

-- Check for missing indexes (tables with high seq_scan)
-- SELECT schemaname, relname, seq_scan, seq_tup_read, idx_scan 
-- FROM pg_stat_user_tables 
-- WHERE schemaname = 'public' AND seq_scan > idx_scan * 10
-- ORDER BY seq_scan DESC;

-- Check index usage on critical tables
-- SELECT * FROM v_index_usage 
-- WHERE tablename IN ('trading_orders', 'order_matches', 'settlements', 'meter_readings')
-- ORDER BY scans DESC;

-- Check for unused indexes (candidates for removal)
-- SELECT schemaname, relname, indexrelname, idx_scan, pg_size_pretty(pg_relation_size(indexrelid))
-- FROM pg_stat_user_indexes 
-- WHERE schemaname = 'public' AND idx_scan < 100
-- ORDER BY pg_relation_size(indexrelid) DESC;
