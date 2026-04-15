-- Migration: Add signatures and payloads to settlements
-- Created: 2026-03-14

ALTER TABLE settlements 
ADD COLUMN IF NOT EXISTS buy_signature TEXT,
ADD COLUMN IF NOT EXISTS sell_signature TEXT,
ADD COLUMN IF NOT EXISTS buy_payload BYTEA,
ADD COLUMN IF NOT EXISTS sell_payload BYTEA;

COMMENT ON COLUMN settlements.buy_signature IS 'On-chain signature for the buy order';
COMMENT ON COLUMN settlements.sell_signature IS 'On-chain signature for the sell order';
COMMENT ON COLUMN settlements.buy_payload IS 'Serialized payload bytes for the buy order';
COMMENT ON COLUMN settlements.sell_payload IS 'Serialized payload bytes for the sell order';
