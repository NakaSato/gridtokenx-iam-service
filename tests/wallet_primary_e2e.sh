#!/usr/bin/env bash
# Live E2E: PATCH /api/v1/me/wallets/{id} primary-wallet contract.
#
# Exercises the only supported wallet mutation:
#
#   GET  /me/wallets                       → custodial wallet present, is_primary
#   PATCH {id} {"is_primary":true}         → 200, wallet stays/becomes primary
#   PATCH {id} {"is_primary":false}        → 400 (only is_primary:true is actionable)
#   PATCH {random-uuid} {"is_primary":true}→ 404 (not owned by caller)
#
# A fresh verified account gets exactly one auto-provisioned custodial wallet,
# which is already primary — so this asserts the idempotent promote + the
# guard rails, without needing a second linked wallet. /me routes require a
# service role → admin. Requires iam-service on :4010 (+ chain bridge for the
# custodial wallet; degrades to skip if none is provisioned).
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

# `|| true` keeps a no-match from tripping `set -o pipefail`.
json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }

# Dev convenience: clear the per-IP register throttle so the suite can register
# a fresh user (the /register 5/hour budget is otherwise exhausted by repeated
# e2e runs from one host).
RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Fresh verified account → JWT ──────────────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="wprim_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Wprim-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "wallet primary suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
AUTH="authorization: Bearer $JWT"

echo "→ GET /me/wallets"
WL=$(curl -s "$BASE/api/v1/me/wallets" -H "$AUTH" -H "$ROLE_HDR")
WID=$(json_field "$WL" id)
if [ -z "$WID" ]; then
  skip "wallet primary suite" "no custodial wallet provisioned (chain bridge down?): ${WL:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "custodial wallet present (id=$WID)"
echo "$WL" | grep -q '"is_primary":true' && ok "auto-provisioned wallet is primary" \
  || bad "fresh wallet not marked primary: ${WL:0:200}"

echo "→ PATCH is_primary:true (idempotent promote)"
R=$(curl -s -w '\n%{http_code}' -X PATCH "$BASE/api/v1/me/wallets/$WID" \
  -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' \
  -d '{"is_primary":true}')
BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "200" ] && ok "PATCH is_primary:true → 200" || bad "PATCH true → $CODE (expected 200)"
echo "$BODY" | grep -q '"is_primary":true' && ok "response wallet is_primary:true" \
  || bad "patched wallet not primary: ${BODY:0:160}"

echo "→ PATCH is_primary:false rejected"
CODE=$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/v1/me/wallets/$WID" \
  -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' \
  -d '{"is_primary":false}')
[ "$CODE" = "400" ] && ok "PATCH is_primary:false → 400 (unsupported no-op)" \
  || bad "PATCH false → $CODE (expected 400)"

echo "→ PATCH a wallet the caller does not own"
RANDID="00000000-0000-4000-8000-000000000000"
CODE=$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/v1/me/wallets/$RANDID" \
  -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' \
  -d '{"is_primary":true}')
[ "$CODE" = "404" ] && ok "PATCH unowned wallet → 404" \
  || bad "PATCH unowned → $CODE (expected 404)"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
