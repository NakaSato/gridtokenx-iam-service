-- Create P2P order side enum
DO $$ BEGIN
    CREATE TYPE p2p_order_side AS ENUM ('buy', 'sell');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Create P2P order status enum
DO $$ BEGIN
    CREATE TYPE p2p_order_status AS ENUM ('open', 'filled', 'cancelled');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Create P2P orders table
CREATE TABLE IF NOT EXISTS p2p_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    side p2p_order_side NOT NULL,
    amount DECIMAL NOT NULL,
    price_per_kwh DECIMAL NOT NULL,
    filled_amount DECIMAL NOT NULL DEFAULT 0,
    status p2p_order_status NOT NULL DEFAULT 'open',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index for faster lookup of open orders
CREATE INDEX IF NOT EXISTS idx_p2p_orders_status ON p2p_orders(status);
CREATE INDEX IF NOT EXISTS idx_p2p_orders_user_id ON p2p_orders(user_id);
