-- Add signature and payload_bytes columns for off-chain order relay
-- Created: February 21, 2026

ALTER TABLE trading_orders ADD COLUMN signature TEXT;
ALTER TABLE trading_orders ADD COLUMN payload_bytes BYTEA;

-- Add a unique index for signature to prevent replays (only for relayed orders)
CREATE UNIQUE INDEX idx_trading_orders_signature ON trading_orders(signature) WHERE signature IS NOT NULL;
