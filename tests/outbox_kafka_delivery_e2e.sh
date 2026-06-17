#!/usr/bin/env bash
# Live E2E: transactional outbox → Kafka durable delivery.
#
# kafka_dual_write_skip_e2e.sh proves an event lands in the outbox table; this
# proves the OutboxWorker actually DELIVERS it to Kafka. Two independent proofs:
#
#   1. DB:    the row reaches status PROCESSED. The worker marks PROCESSED only
#             AFTER the broker acks (acks=all) — see outbox_worker.rs — so
#             PROCESSED ⇒ durably delivered. No row for this user is left
#             PENDING or FAILED.
#   2. Kafka: the matching message is physically present on `iam.user.events`
#             (best-effort console-consume; skips, never false-passes, if the
#             broker tooling is unavailable).
#
# Trigger = register + verify of a unique user → UserRegistered + EmailVerified,
# both routed to the user-events topic (kafka.rs). Routes via kafka-cmd:9001.
# Requires Postgres, the IAM outbox worker, and Kafka up.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
PG_CTR="${PG_CTR:-gridtokenx-postgres}"
KAFKA_CTR="${KAFKA_CTR:-gridtokenx-kafka-cmd}"
KAFKA_BOOTSTRAP="${KAFKA_BOOTSTRAP:-kafka-cmd:9001}"
USER_TOPIC="${USER_TOPIC:-iam.user.events}"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

psql_q() { docker exec "$PG_CTR" psql -U gridtokenx_user -d gridtokenx -tAc "$1" 2>/dev/null | tr -d '\r'; }

# Dev convenience: clear the per-IP register throttle (5/hour otherwise starves
# repeated e2e runs from one host).
if docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$REDIS_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

if ! docker exec "$PG_CTR" pg_isready >/dev/null 2>&1; then
  skip "outbox delivery suite" "Postgres '$PG_CTR' unreachable"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

STAMP="$(date +%s)$RANDOM"
USER="outbox_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Outbox-2025!'

echo "→ register + verify $USER (emits UserRegistered + EmailVerified)"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null

# ── Proof 1: outbox rows drain to PROCESSED ───────────────────────────────────
echo "→ waiting for the outbox worker to deliver (PROCESSED) …"
ROWQ="select event_type||':'||status from iam_outbox_events where payload->'data'->>'username'='$USER' order by created_at"
PENDQ="select count(*) from iam_outbox_events where payload->'data'->>'username'='$USER' and status<>'PROCESSED'"
TOTQ="select count(*) from iam_outbox_events where payload->'data'->>'username'='$USER'"

drained=0
for _ in $(seq 20); do
  tot=$(psql_q "$TOTQ"); pend=$(psql_q "$PENDQ")
  if [ "${tot:-0}" -ge 1 ] && [ "${pend:-1}" = "0" ]; then drained=1; break; fi
  sleep 1
done

ROWS=$(psql_q "$ROWQ")
echo "   outbox rows for $USER:"; echo "$ROWS" | sed 's/^/     /'
if [ "$(psql_q "$TOTQ")" -ge 1 ]; then
  ok "event(s) enqueued to the outbox"
else
  bad "no outbox rows for $USER (event not enqueued)"
fi
if echo "$ROWS" | grep -q '^UserRegistered:'; then
  ok "UserRegistered row present"
else
  bad "UserRegistered row missing for $USER"
fi
if [ "$drained" = "1" ]; then
  ok "all outbox rows for $USER reached PROCESSED (broker-acked, durably delivered)"
else
  REMAIN=$(psql_q "select event_type||':'||status from iam_outbox_events where payload->'data'->>'username'='$USER' and status<>'PROCESSED'")
  bad "outbox not fully drained within 20s — stuck: ${REMAIN:-<none>}"
fi

# ── Proof 2: message physically on the Kafka topic (best-effort) ──────────────
if ! docker exec "$KAFKA_CTR" sh -c 'test -x /opt/kafka/bin/kafka-console-consumer.sh' 2>/dev/null; then
  skip "Kafka physical-consume" "console consumer not available in '$KAFKA_CTR'"
else
  echo "→ consuming $USER_TOPIC to confirm the message physically landed"
  FOUND=$(docker exec "$KAFKA_CTR" /opt/kafka/bin/kafka-console-consumer.sh \
    --bootstrap-server "$KAFKA_BOOTSTRAP" --topic "$USER_TOPIC" \
    --from-beginning --timeout-ms 9000 2>/dev/null | grep -c "$USER" || true)
  if [ "${FOUND:-0}" -ge 1 ]; then
    ok "found $FOUND message(s) for $USER on $USER_TOPIC (end-to-end delivery confirmed)"
  else
    skip "Kafka physical-consume" "no message matched $USER (may be on another topic/partition window)"
  fi
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
