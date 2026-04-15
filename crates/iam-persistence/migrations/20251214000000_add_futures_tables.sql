-- Create futures_products table
CREATE TABLE IF NOT EXISTS futures_products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    symbol VARCHAR(20) NOT NULL UNIQUE,
    base_asset VARCHAR(10) NOT NULL, -- e.g., 'KWH'
    quote_asset VARCHAR(10) NOT NULL, -- e.g., 'GRID'
    contract_size NUMERIC(20, 8) NOT NULL, -- e.g., 1000 KWH per contract
    expiration_date TIMESTAMPTZ NOT NULL,
    current_price NUMERIC(20, 8) NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create futures_orders table
DO $$ BEGIN
    CREATE TYPE futures_order_type AS ENUM ('market', 'limit');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE futures_order_side AS ENUM ('long', 'short');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE futures_order_status AS ENUM ('pending', 'open', 'filled', 'cancelled', 'liquidated');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS futures_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES futures_products(id) ON DELETE CASCADE,
    side futures_order_side NOT NULL,
    order_type futures_order_type NOT NULL,
    quantity NUMERIC(20, 8) NOT NULL, -- Number of contracts
    price NUMERIC(20, 8) NOT NULL, -- Limit price (ignored for market orders)
    leverage INTEGER NOT NULL DEFAULT 1,
    status futures_order_status NOT NULL DEFAULT 'pending',
    filled_quantity NUMERIC(20, 8) DEFAULT 0,
    average_fill_price NUMERIC(20, 8),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create futures_positions table
CREATE TABLE IF NOT EXISTS futures_positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES futures_products(id) ON DELETE CASCADE,
    side futures_order_side NOT NULL, -- 'long' or 'short'
    quantity NUMERIC(20, 8) NOT NULL, -- Number of contracts
    entry_price NUMERIC(20, 8) NOT NULL,
    current_price NUMERIC(20, 8) NOT NULL, -- To calculate PnL
    leverage INTEGER NOT NULL DEFAULT 1,
    margin_used NUMERIC(20, 8) NOT NULL,
    unrealized_pnl NUMERIC(20, 8) DEFAULT 0,
    liquidation_price NUMERIC(20, 8),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(user_id, product_id, side) -- One position per product per side (usually net position, but simple version here)
);

-- Indexes
-- Indexes
CREATE INDEX IF NOT EXISTS idx_futures_products_active ON futures_products(is_active);
CREATE INDEX IF NOT EXISTS idx_futures_orders_user ON futures_orders(user_id);
CREATE INDEX IF NOT EXISTS idx_futures_orders_status ON futures_orders(status);
CREATE INDEX IF NOT EXISTS idx_futures_positions_user ON futures_positions(user_id);

-- Triggers for updated_at
DROP TRIGGER IF EXISTS update_futures_products_updated_at ON futures_products;
CREATE TRIGGER update_futures_products_updated_at
    BEFORE UPDATE ON futures_products
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_futures_orders_updated_at ON futures_orders;
CREATE TRIGGER update_futures_orders_updated_at
    BEFORE UPDATE ON futures_orders
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_futures_positions_updated_at ON futures_positions;
CREATE TRIGGER update_futures_positions_updated_at
    BEFORE UPDATE ON futures_positions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
