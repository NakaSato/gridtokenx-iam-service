#!/usr/bin/env bash
# Live E2E: PATCH /me/wallets/{id} negative space.
#
# wallet_primary_e2e.sh covers the PATCH happy path (promote a secondary to
# primary); wallet_unlink_e2e.sh covers GET/DELETE 404 + primary-delete guard.
# This pins what PATCH rejects (update_wallet, handlers/identity.rs:183):
#
#   PATCH {is_primary:false}      → 400 (only `is_primary:true` is actionable)
#   PATCH {} (empty body)         → 400 (no actionable field)
#   PATCH {is_primary:true} random uuid → 404 (not owned by caller)
#   PATCH non-UUID {id}           → 4xx (path parse / not found)
#
# Only the JWT mint touches register/login; no chain needed. /me routes require
# a service role → admin. Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'
RANDID="00000000-0000-4000-8000-000000000000"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
patch_code() { # <wallet_id> <json-body> → http status
  curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/v1/me/wallets/$1" \
    -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' -d "$2"
}

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Fresh verified account → JWT ──────────────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="wpatch_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Wpatch-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "wallet patch suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
AUTH="authorization: Bearer $JWT"

echo "→ PATCH {is_primary:false} — no-op body rejected"
C=$(patch_code "$RANDID" '{"is_primary":false}')
[ "$C" = "400" ] && ok "is_primary:false → 400 (no actionable field)" \
  || bad "is_primary:false → $C (expected 400)"

echo "→ PATCH {} — empty body rejected"
C=$(patch_code "$RANDID" '{}')
[ "$C" = "400" ] && ok "empty body → 400 (no actionable field)" \
  || bad "empty body → $C (expected 400)"

echo "→ PATCH {is_primary:true} on an unowned/nonexistent id"
C=$(patch_code "$RANDID" '{"is_primary":true}')
[ "$C" = "404" ] && ok "unowned id → 404" || bad "unowned id → $C (expected 404)"

echo "→ PATCH a non-UUID id"
C=$(patch_code "not-a-uuid" '{"is_primary":true}')
case "$C" in
  400|404|422) ok "non-UUID id → $C (rejected)";;
  *)           bad "non-UUID id → $C (expected 400/404/422)";;
esac

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
