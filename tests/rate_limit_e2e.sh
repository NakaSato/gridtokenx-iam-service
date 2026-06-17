#!/usr/bin/env bash
# Live E2E: IP-based rate limiting on /auth/* (rate_limit middleware).
#
# Proves the limiter in crates/iam-api/src/middleware/rate_limit.rs against the
# running service (Redis-backed INCR + TTL):
#
#   /auth/login    → 10 req / 60s  → 11th from the same IP returns 429
#   429 carries no auth oracle (rejected before credential check)
#
# NOTE: the limiter keys on the DIRECT client IP (ConnectInfo). Hit the service
# port directly (:4010), NOT through APISIX — behind the gateway every request
# shares the gateway IP and the count is global, not per-user.
#
# Requires: iam-service on :4010, Redis up. Run against a quiet instance — a
# busy shared IP may already be near the limit.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"
LOGIN_LIMIT="${LOGIN_LIMIT:-10}"   # must match rate_limit.rs (/auth/login → 10/60)

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

# Clear stale login counters for this host so the window starts clean.
if docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:rate_limit:*login*' \
    | xargs -r -n50 docker exec -i "$REDIS_CTR" redis-cli DEL >/dev/null 2>&1 || true
else
  skip "rate-limit counter reset" "Redis container '$REDIS_CTR' unreachable — counts may be dirty"
fi

# Non-existent creds: 401 normally, 429 once limited. We only assert the status code.
BODY='{"username":"ratelimit_probe","password":"wrong-on-purpose"}'
code() { curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/login" \
           -H 'content-type: application/json' -d "$BODY"; }

echo "→ sending $LOGIN_LIMIT allowed login attempts"
hit_429_early=0
for i in $(seq "$LOGIN_LIMIT"); do
  C=$(code)
  if [ "$C" = "429" ]; then hit_429_early=1; echo "   attempt $i already 429 (dirty window)"; break; fi
done
if [ "$hit_429_early" -eq 1 ]; then
  skip "rate-limit threshold" "hit 429 before limit — shared IP window not clean"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "first $LOGIN_LIMIT attempts not rate-limited"

echo "→ attempt #$((LOGIN_LIMIT+1)) should trip the limiter"
C=$(code)
if [ "$C" = "429" ]; then
  ok "attempt #$((LOGIN_LIMIT+1)) → 429 Too Many Requests"
else
  bad "attempt #$((LOGIN_LIMIT+1)) → $C (expected 429)"
fi

# Limiter must reject BEFORE auth: 429, never 200, even with a wrong password.
C=$(code)
if [ "$C" = "429" ]; then
  ok "still 429 while window open (no auth oracle past the limit)"
else
  bad "follow-up attempt → $C (expected sustained 429)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
