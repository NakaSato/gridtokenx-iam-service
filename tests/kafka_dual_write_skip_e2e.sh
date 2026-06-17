#!/usr/bin/env bash
# Live E2E: Kafka dual-write skip for audit-only events.
#
# Proves the event_bus.rs guard (KAFKA_SKIP_EVENT_TYPES) end-to-end against the
# shared Postgres outbox (iam_outbox_events):
#
#   1. High-volume ApiKeyVerified (fired on every gateway VerifyApiKey gRPC call)
#      must NOT be outboxed — else it floods the outbox and head-of-line-blocks
#      real notification events.
#   2. A real notification event (VerificationEmailRequested, via register) MUST
#      still land in the outbox.
#
# Strategy: snapshot outbox counts, drive N VerifyApiKey gRPC calls + one
# register, re-count. ApiKeyVerified delta == 0; notification delta >= 1.
#
# Requires: stack up, grpcurl, and psql access to the IAM Postgres container.
set -euo pipefail

GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
BASE="${IAM_BASE:-http://localhost:4010}"
PG_CTR="${PG_CTR:-gridtokenx-postgres}"
PG_USER="${PG_USER:-gridtokenx_user}"
PG_DB="${PG_DB:-gridtokenx}"
GW_SECRET="${GATEWAY_SECRET:-gridtokenx-gateway-secret-2025}"
GW_HDR="x-gridtokenx-gateway-secret: ${GW_SECRET}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"
N_CALLS="${N_CALLS:-25}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

# outbox_count <event_type>  → integer count in iam_outbox_events
outbox_count() {
  docker exec "$PG_CTR" psql -U "$PG_USER" -d "$PG_DB" -tAc \
    "SELECT count(*) FROM iam_outbox_events WHERE event_type = '$1'" 2>/dev/null | tr -d '[:space:]'
}

# Preconditions.
if ! docker exec "$PG_CTR" psql -U "$PG_USER" -d "$PG_DB" -tAc 'SELECT 1' >/dev/null 2>&1; then
  skip "Kafka dual-write skip suite" "Postgres container '$PG_CTR' not reachable"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
if ! docker exec "$PG_CTR" psql -U "$PG_USER" -d "$PG_DB" -tAc \
     "SELECT to_regclass('iam_outbox_events')" 2>/dev/null | grep -q iam_outbox_events; then
  skip "Kafka dual-write skip suite" "iam_outbox_events table absent (outbox disabled?)"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

AKV_BEFORE=$(outbox_count ApiKeyVerified)
VER_BEFORE=$(outbox_count VerificationEmailRequested)
echo "→ outbox baseline: ApiKeyVerified=$AKV_BEFORE VerificationEmailRequested=$VER_BEFORE"

# ── Drive ApiKeyVerified: N VerifyApiKey gRPC calls (audit event each) ────────
if command -v grpcurl >/dev/null 2>&1 && [[ -f "$PROTO_DIR/identity.proto" ]]; then
  echo "→ firing $N_CALLS VerifyApiKey gRPC calls"
  for _ in $(seq "$N_CALLS"); do
    grpcurl -max-time 10 -plaintext \
      -import-path "$PROTO_DIR" -proto identity.proto \
      -H 'x-gridtokenx-role: aggregator-bridge' -H "$GW_HDR" \
      -d '{"key":"gtx_probe_invalid_key"}' \
      "$GRPC_ADDR" identity.IdentityService/VerifyApiKey >/dev/null 2>&1 || true
  done
else
  skip "VerifyApiKey driver" "grpcurl/identity.proto missing — ApiKeyVerified path not exercised"
fi

# ── Drive a real notification event: register a fresh user ────────────────────
STAMP="$(date +%s)$RANDOM"
echo "→ register fresh user to emit VerificationEmailRequested"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"kfk_${STAMP}\",\"email\":\"kfk_${STAMP}@example.com\",\"password\":\"GridTokenX-\$2024-@Kafka\"}" >/dev/null || true

sleep 1
AKV_AFTER=$(outbox_count ApiKeyVerified)
VER_AFTER=$(outbox_count VerificationEmailRequested)
echo "→ outbox after: ApiKeyVerified=$AKV_AFTER VerificationEmailRequested=$VER_AFTER"

# ── Assertions ────────────────────────────────────────────────────────────────
if [ "$AKV_AFTER" -eq "$AKV_BEFORE" ]; then
  ok "ApiKeyVerified NOT outboxed (Δ=0 across $N_CALLS calls)"
else
  bad "ApiKeyVerified leaked into outbox (Δ=$((AKV_AFTER-AKV_BEFORE))) — dual-write skip broken"
fi

if [ "$VER_AFTER" -gt "$VER_BEFORE" ]; then
  ok "VerificationEmailRequested still outboxed (Δ=$((VER_AFTER-VER_BEFORE)))"
else
  bad "VerificationEmailRequested missing from outbox — real events not dual-writing"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
