-- Transactional outbox for the trading service event pipeline.
--
-- The trading service's OutboxWorker polls this table every 5s; without it the
-- worker logs `relation "outbox_events" does not exist` on every tick and the
-- OrderMatched/OrderUpdate/Settlement events it enqueues are dropped. IAM owns
-- the shared Postgres schema (trading-service has no migrations of its own), so
-- the table is defined here alongside the trading_orders tables.
--
-- Columns mirror PostgresOutboxRepository::OutboxEventDb (SELECT * FromRow):
-- id, event_type, payload, status, attempts, last_attempt_at, created_at.
-- status values are upper-case ('PENDING'/'PROCESSED'/'FAILED') per the repo.

CREATE TABLE IF NOT EXISTS outbox_events (
    id              UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    event_type      TEXT        NOT NULL,
    payload         JSONB       NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'PENDING',
    attempts        INTEGER     NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- get_pending_events: WHERE status = 'PENDING' ORDER BY created_at ASC.
CREATE INDEX IF NOT EXISTS idx_outbox_events_pending
    ON outbox_events (status, created_at);

COMMENT ON TABLE outbox_events IS
    'Transactional outbox for trading-service domain events (OutboxWorker drains it).';
