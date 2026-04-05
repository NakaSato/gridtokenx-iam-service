-- Migration: Add next_retry_at to settlements for exponential backoff
-- Created: 2026-03-12

ALTER TABLE settlements 
ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ;

-- Add index for efficient polling of pending retries
CREATE INDEX IF NOT EXISTS idx_settlements_next_retry_at ON settlements(next_retry_at) 
WHERE status = 'pending';

COMMENT ON COLUMN settlements.next_retry_at IS 'Scheduled time for the next settlement attempt';
