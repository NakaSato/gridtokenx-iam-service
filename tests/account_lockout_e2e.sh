#!/usr/bin/env bash
# Live E2E: account lockout after repeated failed logins.
#
# login() (auth_service.rs) counts failures in Redis `iam:login_attempts:<user>`
# and, at the 5th, arms `iam:account_lock:<user>` (TTL 900s) and refuses further
# logins — even with the CORRECT password — until it expires. This pins:
#
#   attempts 1–4 (wrong pw)        → 401 invalid credentials
#   attempt 5    (wrong pw)        → locked (error code 1006 AccountLocked)
#   attempt with the RIGHT pw      → still locked (lock gates before credential check)
#   Redis                          → iam:account_lock:<user> present, 0 < TTL ≤ 900
#
# NOTE ON STATUS: AccountLocked is mapped to HTTP 423 LOCKED (error/types.rs
# status_code()). It previously fell through the WithCode(_) catch-all to 500 —
# a mislabel, since a lockout is a client condition, not a server fault, and 500s
# pollute 5xx alarms. This suite asserts the SEMANTIC outcome via the body's
# error code (1006) and only reports the raw status, so it passes against either
# an old (500) or fixed (423) binary; the printed status is the canary.
#
# Lockout = 5 failures, which sits under the /auth/login 10/60 rate limit, so the
# login rate counter is cleared first to guarantee the 6 calls all land. Requires
# iam-service on :4010 + Redis.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

# Clear per-IP register + login throttles so the failed-login sequence isn't
# pre-empted by a 429 (dev convenience).
if docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:rate_limit:*login*' \
    | xargs -r -n50 docker exec -i "$REDIS_CTR" redis-cli DEL >/dev/null 2>&1 || true
  docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$REDIS_CTR" redis-cli DEL >/dev/null 2>&1 || true
else
  skip "throttle reset" "Redis '$REDIS_CTR' unreachable — login rate counter may pre-empt the lockout"
fi

STAMP="$(date +%s)$RANDOM"
USER="lockout_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Lockout-2025!'

echo "→ register + verify $USER"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null

# login <password> → "<body>\n<code>"
login() {
  curl -s -w '\n%{http_code}' -X POST "$BASE/api/v1/auth/login" \
    -H 'content-type: application/json' \
    -d "{\"username\":\"$USER\",\"password\":\"$1\"}"
}
is_locked() { echo "$1" | grep -q '"code_number":1006'; }

echo "→ 4 wrong-password attempts (below the lock threshold)"
for i in 1 2 3 4; do
  R=$(login 'wrong-on-purpose'); CODE=$(echo "$R" | tail -1)
  [ "$CODE" = "401" ] && ok "attempt $i → 401 invalid credentials" \
    || bad "attempt $i → $CODE (expected 401)"
done

echo "→ 5th wrong-password attempt should trip the lock"
R=$(login 'wrong-on-purpose'); BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
if is_locked "$BODY"; then
  ok "attempt 5 → AccountLocked (code 1006); raw HTTP $CODE"
  [ "$CODE" = "500" ] || echo "   ⓘ note: HTTP $CODE (was 500 at authoring — status mapping may have been fixed)"
else
  bad "attempt 5 not locked: ${BODY:0:160}"
fi

echo "→ correct password must ALSO be refused while locked"
R=$(login "$PW"); BODY=$(echo "$R" | sed '$d')
if is_locked "$BODY"; then
  ok "correct password → still AccountLocked (lock gates before credential check)"
else
  bad "lock did not gate a valid login: ${BODY:0:160}"
fi

echo "→ Redis lock key is armed with a bounded TTL"
if docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  LKEY="iam:account_lock:$USER"
  if [ "$(docker exec "$REDIS_CTR" redis-cli EXISTS "$LKEY" | tr -d '\r')" = "1" ]; then
    TTL=$(docker exec "$REDIS_CTR" redis-cli TTL "$LKEY" | tr -d '\r')
    if [ "$TTL" -gt 0 ] && [ "$TTL" -le 900 ]; then
      ok "$LKEY armed (ttl=${TTL}s ≤ 900)"
    else
      bad "$LKEY TTL out of range: $TTL"
    fi
  else
    bad "$LKEY not set after lockout"
  fi
else
  skip "Redis lock-key check" "Redis '$REDIS_CTR' unreachable"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
