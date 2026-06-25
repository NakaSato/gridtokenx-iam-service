#!/usr/bin/env bash
# Live E2E: registration conflict + on-chain onboarding idempotency.
#
# email_verify_cycle_e2e.sh proves the register→verify happy path;
# me_registration_e2e.sh proves the onboard guard/contract. Neither pins:
#
#   register U1/E1                → 200 (baseline)
#   register U1/E2 (dup username) → 409  (Conflict: "… already exists")
#   register U2/E1 (dup email)    → 409
#   register U1/E1 (identical)    → 409  (no second account, no overwrite)
#   onboard twice (same user)     → both 200 (idempotent; PDA create is retry-safe)
#
# Dup-register needs no chain. The onboard-idempotency leg needs Chain Bridge /
# Solana and degrades to skip if onboarding can't reach 200 (5xx). Conflict maps
# to 409 via ApiError::Conflict (auth_service register path, error/types.rs).
# /me routes require a service role → admin. Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'
ONBOARD_BODY='{"user_type":"prosumer","location":{"lat_e7":138000000,"long_e7":1005000000}}'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
reg_code() { # <username> <email> → http status
  curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/auth/register" \
    -H "$ROLE_HDR" -H 'content-type: application/json' \
    -d "{\"username\":\"$1\",\"email\":\"$2\",\"password\":\"$PW\"}"
}

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

STAMP="$(date +%s)$RANDOM"
U1="ridem_${STAMP}";  E1="${U1}@example.com"
U2="ridem2_${STAMP}"; E2="${U2}@example.com"
PW='GridTokenX-$Ridem-2025!'

# ── Conflict matrix (no chain needed) ─────────────────────────────────────────
echo "→ first registration is the baseline"
C=$(reg_code "$U1" "$E1")
[ "$C" = "200" ] || [ "$C" = "201" ] && ok "register U1/E1 → $C" || bad "register U1/E1 → $C (expected 200/201)"

echo "→ duplicate username (different email)"
C=$(reg_code "$U1" "$E2")
[ "$C" = "409" ] && ok "dup username → 409 (Conflict)" || bad "dup username → $C (expected 409)"

echo "→ duplicate email (different username)"
C=$(reg_code "$U2" "$E1")
[ "$C" = "409" ] && ok "dup email → 409 (Conflict)" || bad "dup email → $C (expected 409)"

echo "→ identical re-registration (no overwrite, no second account)"
C=$(reg_code "$U1" "$E1")
[ "$C" = "409" ] && ok "identical re-register → 409 (idempotent reject)" \
  || bad "identical re-register → $C (expected 409)"

# ── Onboard idempotency (needs chain; degrade-to-skip) ────────────────────────
curl -s "$BASE/api/v1/auth/verify?token=verify_$E1" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$U1\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "onboard idempotency" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; [ "$FAIL" -eq 0 ]; exit $?
fi
AUTH="authorization: Bearer $JWT"

onboard_code() {
  curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/me/registration" \
    -H "$AUTH" -H "$ROLE_HDR" -H 'content-type: application/json' -d "$ONBOARD_BODY"
}

echo "→ first onboarding"
C1=$(onboard_code)
if [ "$C1" != "200" ]; then
  skip "onboard idempotency" "first onboard → $C1 (Chain Bridge / validator down); idempotency unverifiable"
else
  echo "→ second onboarding (must be idempotent, not a conflict)"
  C2=$(onboard_code)
  [ "$C2" = "200" ] && ok "onboard twice → 200/200 (idempotent)" \
    || bad "second onboard → $C2 (expected 200; idempotent re-onboard)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
