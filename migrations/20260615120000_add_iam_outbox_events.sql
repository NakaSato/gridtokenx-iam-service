-- Transactional outbox for IAM domain events — durable Kafka delivery.
--
-- IAM's EventBus writes events to Redis Streams synchronously (reliable,
-- in-cluster) but dual-writes to Kafka fire-and-forget: a broker blip silently
-- dropped events (e.g. VerificationEmailRequested -> verification mail never
-- sent). This table makes the Kafka leg durable — EventBus enqueues the event
-- here, and the IAM OutboxWorker drains it, delivers to Kafka with retry, and
-- marks each row PROCESSED (delivered) or FAILED (quarantined after N attempts).
--
-- Deliberately SEPARATE from `outbox_events` (the trading-service table on this
-- shared Postgres): trading's OutboxWorker does `SELECT * WHERE status='PENDING'`
-- with no service filter and deserializes every row as a trading-core Event, so
-- IAM rows written there would poison its batch and loop forever. Keep apart.
--
-- Columns mirror iam-persistence `OutboxRow` (runtime sqlx query_as):
-- id, event_type, payload, status, attempts, last_attempt_at, created_at.
-- status values are upper-case ('PENDING'/'PROCESSED'/'FAILED').

CREATE TABLE IF NOT EXISTS iam_outbox_events (
    id              UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    event_type      TEXT        NOT NULL,
    payload         JSONB       NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'PENDING',
    attempts        INTEGER     NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- fetch_pending: WHERE status = 'PENDING' ORDER BY created_at ASC.
CREATE INDEX IF NOT EXISTS idx_iam_outbox_events_pending
    ON iam_outbox_events (status, created_at);

COMMENT ON TABLE iam_outbox_events IS
    'Transactional outbox for IAM domain events; IAM OutboxWorker drains to Kafka.';
