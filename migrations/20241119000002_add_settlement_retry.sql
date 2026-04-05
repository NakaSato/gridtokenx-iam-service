-- Add retry_count column to settlements table for retry tracking
ALTER TABLE settlements 
ADD COLUMN IF NOT EXISTS retry_count INTEGER DEFAULT 0;

-- Add index for retry tracking
CREATE INDEX IF NOT EXISTS idx_settlements_retry_count ON settlements(retry_count);
