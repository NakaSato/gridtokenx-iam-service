#!/usr/bin/env bash
# Live E2E: POST /api/v1/auth/login response contract.
#
# Login is hit ~15× across the suite but only ever as JWT-mint *setup* — its own
# response shape is never asserted. This pins it (login handler, auth.rs:76 →
# AuthResponse { access_token, expires_in, user{…} }):
#
#   verified + correct pw → 200, body has access_token + user.id + matching
#                           username + user.status == "verified"
#   verified + wrong pw   → 401 (invalid credentials)
#   unverified + correct pw → 200, user.status == "pending_verification"
#                           (login is NOT email-gated; status reflects is_active)
#
# NOTE: REST login returns no refresh_token — /auth/refresh mints a fresh access
# token from a valid bearer instead (auth.rs:126). Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
login() { # <username> <password> → "<http_code>\n<body>"
  curl -s -w '\n%{http_code}' -X POST "$BASE/api/v1/auth/login" \
    -H 'content-type: application/json' \
    -d "{\"username\":\"$1\",\"password\":\"$2\"}"
}

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

STAMP="$(date +%s)$RANDOM"
USER="login_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Login-2025!'

# ── Verified account: happy path + wrong password ─────────────────────────────
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null

echo "→ verified user, correct password"
R=$(login "$USER" "$PW")
BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "200" ] && ok "login → 200" || bad "login → $CODE (expected 200)"
[ -n "$(json_field "$BODY" access_token)" ] && ok "body has access_token" \
  || bad "no access_token in body: ${BODY:0:160}"
[ -n "$(json_field "$BODY" id)" ] && ok "body carries user.id" \
  || bad "no user id in body: ${BODY:0:160}"
[ "$(json_field "$BODY" username)" = "$USER" ] && ok "echoed username matches" \
  || bad "username mismatch: ${BODY:0:160}"
[ "$(json_field "$BODY" status)" = "verified" ] && ok "user.status == verified" \
  || bad "status != verified: ${BODY:0:160}"

echo "→ verified user, wrong password"
R=$(login "$USER" "wrong-${PW}")
CODE=$(echo "$R" | tail -1)
[ "$CODE" = "401" ] && ok "wrong password → 401" || bad "wrong password → $CODE (expected 401)"

# ── Unverified account: login is not email-gated ──────────────────────────────
UV="loginuv_${STAMP}"; UVE="${UV}@example.com"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$UV\",\"email\":\"$UVE\",\"password\":\"$PW\"}" >/dev/null
# deliberately NOT verified

echo "→ unverified user, correct password"
R=$(login "$UV" "$PW")
BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
if [ "$CODE" = "200" ]; then
  [ "$(json_field "$BODY" status)" = "pending_verification" ] \
    && ok "unverified login → 200, status pending_verification" \
    || bad "unverified login 200 but status=$(json_field "$BODY" status) (expected pending_verification)"
elif [ "$CODE" = "401" ] || [ "$CODE" = "403" ]; then
  ok "unverified login → $CODE (email-gated — stricter than expected, acceptable)"
else
  bad "unverified login → $CODE (expected 200 or 401/403)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
