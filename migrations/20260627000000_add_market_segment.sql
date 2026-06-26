-- Migration: Add market_segment enum + column to trading_orders
-- Purpose: trading-service routes each order to one of two matching mechanisms —
-- the continuous CDA matcher (realtime) or the 15-minute uniform-price clearing
-- (interval). The Rust MarketSegment enum (lowercase labels realtime/interval)
-- maps via sqlx to this Postgres enum. Without the type + column the segment
-- cannot round-trip through the DB: an interval order is read back as the default
-- realtime and the CDA matcher picks it up instead of the clearing worker.

-- 1. Create the enum (idempotent)
DO $$ BEGIN
    CREATE TYPE market_segment AS ENUM ('realtime', 'interval');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 2. Add the column. Default 'realtime' matches MarketSegment::default(), so every
-- existing order keeps its current (continuous-matcher) behaviour.
ALTER TABLE trading_orders
    ADD COLUMN IF NOT EXISTS market_segment market_segment NOT NULL DEFAULT 'realtime';

-- 3. Comment
COMMENT ON COLUMN trading_orders.market_segment IS 'Matching mechanism: realtime (continuous CDA matcher) or interval (15-min uniform-price clearing)';
