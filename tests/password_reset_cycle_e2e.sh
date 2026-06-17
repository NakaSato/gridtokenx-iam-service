#!/usr/bin/env bash
# Live E2E: full password-reset cycle (forgot → reset → re-login).
#
# auth_flow_test.sh only checks that forgot-password responds; this proves the
# whole credential rotation actually takes effect end-to-end:
#
#   register → verify → login(old pw) OK
#   forgot-password  → a single-use reset token is minted in Redis (15m TTL)
#   reset-password(token, new pw) → 200
#   login(old pw) → 401   (old credential revoked)
#   login(new pw) → 200   (new credential active)
#   reset-password(same token) → 400  (token is single-use, deleted on use)
#
# The reset token is normally delivered by email (Mailpit). To stay
# infra-light this reads it straight from Redis: forgot_password stores
# `iam:password_reset:<token>` = <email>. Requires iam-service :4010 + Redis.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

STAMP="$(date +%s)$RANDOM"
USER="pwreset_${STAMP}"
EMAIL="${USER}@example.com"
OLD_PASS='GridTokenX-$Old-2024!'
NEW_PASS='GridTokenX-$New-2025!'

# Dev convenience: clear the per-IP register throttle (the /register 5/hour
# budget is otherwise exhausted by repeated e2e runs from one host).
if docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$REDIS_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

login_code() { # <password> → http status
  curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/login" \
    -H 'content-type: application/json' \
    -d "{\"username\":\"$USER\",\"password\":\"$1\"}"
}

echo "→ register + verify $EMAIL"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$OLD_PASS\"}" >/dev/null
# dev verify shortcut (ENVIRONMENT != production) activates the account.
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null

C=$(login_code "$OLD_PASS")
[ "$C" = "200" ] && ok "login with old password → 200" || bad "login old password → $C (expected 200)"

echo "→ forgot-password"
R=$(curl -s -X POST "$BASE/api/v1/auth/forgot-password" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\"}")
echo "$R" | grep -qiE 'sent|reset link' && ok "forgot-password → generic sent response" \
  || bad "forgot-password body: $R"

if ! docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  skip "reset cycle" "Redis '$REDIS_CTR' unreachable — cannot recover reset token"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; [ "$FAIL" -eq 0 ]; exit $?
fi

# Find the reset token whose stored value is OUR email (avoids cross-test races).
TOKEN=""
while read -r key; do
  [ -z "$key" ] && continue
  key="${key%$'\r'}"
  # Values are serde_json-encoded, so they arrive wrapped in literal quotes.
  val=$(docker exec "$REDIS_CTR" redis-cli GET "$key" | tr -d '\r"')
  if [ "$val" = "$EMAIL" ]; then  # EMAIL is already lowercase; store side lowercases too
    TOKEN="${key#iam:password_reset:}"; break
  fi
done < <(docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:password_reset:*')

if [ -z "$TOKEN" ]; then
  bad "no reset token in Redis for $EMAIL"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 1
fi
ok "reset token minted in Redis (single-use, TTL-bound)"

echo "→ reset-password with token"
C=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/reset-password" \
  -H 'content-type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"new_password\":\"$NEW_PASS\"}")
[ "$C" = "200" ] && ok "reset-password → 200" || bad "reset-password → $C (expected 200)"

echo "→ verify credential rotation"
C=$(login_code "$OLD_PASS")
[ "$C" = "401" ] && ok "login with OLD password → 401 (revoked)" || bad "login old password → $C (expected 401)"
C=$(login_code "$NEW_PASS")
[ "$C" = "200" ] && ok "login with NEW password → 200 (active)" || bad "login new password → $C (expected 200)"

echo "→ token replay must fail (single-use)"
C=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/reset-password" \
  -H 'content-type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"new_password\":\"$NEW_PASS\"}")
[ "$C" = "400" ] && ok "reused reset token → 400 (consumed)" || bad "reused token → $C (expected 400)"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
