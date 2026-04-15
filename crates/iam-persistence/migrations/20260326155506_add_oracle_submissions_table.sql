-- Create oracle_submissions table for tracking oracle bridge submissions
-- This table provides an audit trail for all meter readings submitted via the Oracle Bridge

CREATE TABLE IF NOT EXISTS oracle_submissions (
    id BIGSERIAL PRIMARY KEY,
    reading_id UUID NOT NULL UNIQUE,
    meter_id UUID NOT NULL,
    meter_serial VARCHAR(255) NOT NULL,
    user_id UUID NOT NULL,
    wallet_address VARCHAR(255) NOT NULL,
    zone_id INTEGER,
    kwh DECIMAL(12, 9) NOT NULL DEFAULT 0,
    energy_generated DECIMAL(12, 9),
    energy_consumed DECIMAL(12, 9),
    reading_timestamp BIGINT NOT NULL,
    signature VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'submitted',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

-- Create indexes for efficient querying
CREATE INDEX idx_oracle_submissions_reading_id ON oracle_submissions(reading_id);
CREATE INDEX idx_oracle_submissions_meter_id ON oracle_submissions(meter_id);
CREATE INDEX idx_oracle_submissions_meter_serial ON oracle_submissions(meter_serial);
CREATE INDEX idx_oracle_submissions_user_id ON oracle_submissions(user_id);
CREATE INDEX idx_oracle_submissions_signature ON oracle_submissions(signature);
CREATE INDEX idx_oracle_submissions_status ON oracle_submissions(status);
CREATE INDEX idx_oracle_submissions_reading_timestamp ON oracle_submissions(reading_timestamp DESC);
CREATE INDEX idx_oracle_submissions_created_at ON oracle_submissions(created_at DESC);

-- Add comment for documentation
COMMENT ON TABLE oracle_submissions IS 'Audit trail for meter readings submitted via Oracle Bridge to Solana blockchain';
COMMENT ON COLUMN oracle_submissions.reading_id IS 'Unique identifier for the reading from Oracle Bridge';
COMMENT ON COLUMN oracle_submissions.signature IS 'Solana transaction signature for on-chain submission';
COMMENT ON COLUMN oracle_submissions.status IS 'Submission status: submitted, confirmed, failed';
COMMENT ON COLUMN oracle_submissions.reading_timestamp IS 'Unix timestamp of the meter reading';
