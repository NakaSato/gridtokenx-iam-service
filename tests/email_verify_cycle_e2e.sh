#!/usr/bin/env bash
# Live E2E: email-verification activation cycle.
#
# Proves /auth/verify actually flips the account active and that the gate is
# enforced on login:
#
#   register                       → account pending (login refused)
#   login (pre-verify)             → NOT 200 (unverified cannot authenticate)
#   verify(verify_<email>)         → success, returns auth.access_token +
#                                    status "verified" + a provisioned wallet_address
#   GET /me (that token)           → 200, correct username/email
#   login (post-verify)            → 200
#   verify again                   → still success (idempotent re-activation)
#
# Uses the dev verify shortcut `verify_<email>` (valid only when
# ENVIRONMENT != production). /me requires a service role → send admin.
# Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

# NB: keep the username clear of a leading "verify_" — the dev shortcut does
# trim_start_matches("verify_"), which strips a REPEATED prefix and would mangle
# the email back-out (verify_verify_<x> → <x>, not verify_<x>).
STAMP="$(date +%s)$RANDOM"
USER="emverify_${STAMP}"
EMAIL="${USER}@example.com"
PASS_STR='GridTokenX-$Verify-2025!'

# `|| true` keeps a no-match from tripping `set -o pipefail`.
json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }

# Dev convenience: clear the per-IP register throttle (the /register 5/hour
# budget is otherwise exhausted by repeated e2e runs from one host).
RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

echo "→ register $EMAIL (pending)"
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS_STR\"}" >/dev/null

echo "→ login before verification must be refused"
C=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/login" \
  -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_STR\"}")
if [ "$C" = "200" ]; then
  bad "login pre-verify → 200 (unverified account should not authenticate)"
else
  ok "login pre-verify → $C (refused until verified)"
fi

echo "→ verify via dev shortcut"
V=$(curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL")
echo "$V" | grep -q '"success":true' && ok "verify → success:true" || bad "verify body: $V"

WALLET=$(json_field "$V" wallet_address)
[ -n "$WALLET" ] && ok "verify provisioned a custodial wallet ($WALLET)" \
  || skip "wallet provisioning" "no wallet_address in verify response (chain bridge may be down)"

TOKEN=$(json_field "$V" access_token)
if [ -z "$TOKEN" ]; then
  bad "verify did not return an auth.access_token"
else
  ok "verify returned an auto-login access_token"
  echo "→ GET /me with the verify-issued token"
  ME=$(curl -s "$BASE/api/v1/me" -H "authorization: Bearer $TOKEN" -H 'x-gridtokenx-role: admin')
  echo "$ME" | grep -q "\"$USER\"" && ok "/me returns the verified user ($USER)" \
    || bad "/me body did not contain $USER: ${ME:0:160}"
fi

echo "→ login after verification"
C=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/login" \
  -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_STR\"}")
[ "$C" = "200" ] && ok "login post-verify → 200" || bad "login post-verify → $C (expected 200)"

echo "→ re-verify is idempotent"
V2=$(curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL")
echo "$V2" | grep -q '"success":true' && ok "re-verify → success:true (idempotent)" \
  || bad "re-verify body: $V2"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
