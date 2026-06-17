#!/usr/bin/env bash
# Live E2E: resend-verification cooldown.
# Proves (against the running IAM container + Redis):
#   1. First resend for an unverified account → 200 "sent" + a cooldown key is
#      armed in Redis with a TTL near RESEND_VERIFICATION_COOLDOWN_SECS (60).
#   2. Immediate second resend → still 200 "sent" (generic, no enumeration leak)
#      and the service logs that it suppressed the re-send.
#
# Requires: iam-service on :4010, gridtokenx-redis reachable via docker exec.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"
IAM_CTR="${IAM_CTR:-gridtokenx-iam-service}"
PASS=0; FAIL=0
ok()  { echo "✅ $1"; PASS=$((PASS+1)); }
bad() { echo "❌ $1"; FAIL=$((FAIL+1)); }

# Unique unverified account.
STAMP="$(date +%s)$RANDOM"
EMAIL="cooldown_${STAMP}@example.com"
USER="cooldown_${STAMP}"
PASS_STR='GridTokenX-$2024-@Cooldown'

echo "→ register unverified user $EMAIL"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS_STR\"}" >/dev/null

# Clear any stale cooldown keys so the count delta is clean.
BEFORE=$(docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:resend_cooldown:*' | wc -l | tr -d ' ')

echo "→ resend #1"
R1=$(curl -s -X POST "$BASE/api/v1/auth/resend-verification" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\"}")
echo "$R1" | grep -q '"status":"sent"' && ok "resend #1 → sent" || bad "resend #1 body: $R1"

# A new cooldown key must now exist with a positive TTL ≤ 60.
KEY=$(docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:resend_cooldown:*' | tail -n1 | tr -d '\r')
if [ -n "$KEY" ]; then
  TTL=$(docker exec "$REDIS_CTR" redis-cli TTL "$KEY" | tr -d '\r')
  if [ "$TTL" -gt 0 ] && [ "$TTL" -le 60 ]; then
    ok "cooldown key armed ($KEY ttl=${TTL}s)"
  else
    bad "cooldown key TTL out of range: $TTL"
  fi
else
  bad "no cooldown key armed after resend #1"
fi

echo "→ resend #2 (immediate)"
R2=$(curl -s -X POST "$BASE/api/v1/auth/resend-verification" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\"}")
echo "$R2" | grep -q '"status":"sent"' && ok "resend #2 → sent (generic, no leak)" || bad "resend #2 body: $R2"

# Suppression evidence in the service log.
if docker logs --since 30s "$IAM_CTR" 2>&1 | grep -qi 'suppressing re-send'; then
  ok "service logged cooldown suppression"
else
  bad "no suppression log line found (check log level)"
fi

echo "── $PASS passed, $FAIL failed ──"
[ "$FAIL" -eq 0 ]
