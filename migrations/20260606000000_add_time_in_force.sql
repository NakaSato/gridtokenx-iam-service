-- Migration: Add time_in_force enum + column to trading_orders
-- Purpose: trading-service inserts an order.time_in_force value mapped (sqlx) to the
-- Postgres enum `time_in_force` (Rust TimeInForce, lowercase labels gtc/fok/ioc). The
-- type and column were never created, so order placement fails at runtime with
-- `type "time_in_force" does not exist`.

-- 1. Create the enum (idempotent)
DO $$ BEGIN
    CREATE TYPE time_in_force AS ENUM ('gtc', 'fok', 'ioc');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 2. Add the column. Default 'gtc' (Good-Til-Cancelled) matches TimeInForce::default().
ALTER TABLE trading_orders
    ADD COLUMN IF NOT EXISTS time_in_force time_in_force NOT NULL DEFAULT 'gtc';

-- 3. Comment
COMMENT ON COLUMN trading_orders.time_in_force IS 'Order time-in-force policy: gtc (Good-Til-Cancelled), fok (Fill-or-Kill), ioc (Immediate-or-Cancel)';

-- 4. limit_price: trading-service TradingOrderDb (SELECT *) maps a nullable limit_price
-- column that was likewise never created, so list/get order queries fail with
-- `no column found for name: limit_price`. Nullable; price_per_kwh remains the working price.
ALTER TABLE trading_orders
    ADD COLUMN IF NOT EXISTS limit_price NUMERIC(20,8);
COMMENT ON COLUMN trading_orders.limit_price IS 'Optional explicit limit price for the order (conditional/advanced order types); base price is price_per_kwh';
