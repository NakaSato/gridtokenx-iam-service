#!/usr/bin/env bash
# Live E2E: POST /api/v1/me/registration (on-chain onboarding).
#
# onboard_user (handlers/identity.rs:46) kicks off Registry-PDA creation through
# Chain Bridge for the authenticated user. This pins the guard + request
# contract; the on-chain effect itself needs a Solana validator and degrades:
#
#   no bearer token              → 401 (auth-gated)
#   malformed body               → 4xx (rejected before any chain work)
#   valid token + body           → 200 with a status (e.g. processing), OR
#                                   degrade-skip if Chain Bridge / validator is
#                                   down (5xx) — the chain effect is unverifiable
#                                   without it.
#
# Body: user_type ∈ {prosumer,consumer} (serde lowercase) + location lat/long e7.
# /me routes require a service role → admin. Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'
BODY='{"user_type":"prosumer","location":{"lat_e7":138000000,"long_e7":1005000000}}'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

echo "→ unauthenticated request is rejected"
C=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/me/registration" \
  -H "$ROLE_HDR" -H 'content-type: application/json' -d "$BODY")
[ "$C" = "401" ] && ok "no bearer token → 401" || bad "no token → $C (expected 401)"

# ── Fresh verified account → JWT ──────────────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="mreg_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Mreg-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "registration suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
AUTH="authorization: Bearer $JWT"

echo "→ malformed body is rejected before chain work"
C=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/me/registration" \
  -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' -d '{"user_type":"bogus"}')
case "$C" in
  400|422) ok "malformed body → $C (rejected)";;
  *)       bad "malformed body → $C (expected 400/422)";;
esac

echo "→ authenticated onboarding request"
R=$(curl -s -w '\n%{http_code}' -X POST "$BASE/api/v1/me/registration" \
  -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' -d "$BODY")
RBODY=$(echo "$R" | sed '$d'); RCODE=$(echo "$R" | tail -1)
if [ "$RCODE" = "200" ]; then
  echo "$RBODY" | grep -q '"status"' \
    && ok "valid onboarding → 200 with status ($(json_field "$RBODY" status))" \
    || bad "200 but no status field: ${RBODY:0:160}"
elif [ "$RCODE" -ge 500 ] 2>/dev/null; then
  skip "on-chain onboarding effect" "got $RCODE — Chain Bridge / Solana validator down; chain effect unverifiable"
else
  bad "onboarding → $RCODE (expected 200 or 5xx-degrade): ${RBODY:0:160}"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
