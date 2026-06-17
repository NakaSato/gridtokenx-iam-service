#!/usr/bin/env bash
# Live E2E: bearer-token rejection on the REST auth guard.
#
# auth_refresh_e2e.sh covers the refresh happy-path; this pins the negative
# space — that a forged/expired/absent token cannot pass the /me guard:
#
#   valid JWT                     → 200 (baseline)
#   tampered signature            → 401 (HMAC mismatch)
#   tampered payload (re-b64)     → 401 (signature no longer covers claims)
#   garbage / non-JWT             → 401
#   absent Authorization header   → 401
#
# /me requires a service role → admin; the bearer token is the user identity.
# Requires iam-service on :4010 (+ Redis for the register throttle reset).
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
code_for() { # <auth-header-value-or-empty>
  if [ -z "$1" ]; then
    curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me" -H "$ROLE_HDR"
  else
    curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me" -H "authorization: Bearer $1" -H "$ROLE_HDR"
  fi
}

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Mint a real JWT ───────────────────────────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="jtam_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Jtam-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "jwt tamper suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "minted a real JWT via REST login"

echo "→ valid JWT baseline"
C=$(code_for "$JWT")
[ "$C" = "200" ] && ok "valid JWT → 200" || bad "valid JWT → $C (expected 200)"

# JWT = header.payload.signature
HDR="${JWT%%.*}"; REST="${JWT#*.}"; PAYLOAD="${REST%%.*}"; SIG="${REST#*.}"

echo "→ tampered signature"
TSIG="${SIG}xyz"
C=$(code_for "${HDR}.${PAYLOAD}.${TSIG}")
[ "$C" = "401" ] && ok "tampered signature → 401" || bad "tampered sig → $C (expected 401)"

echo "→ tampered payload (flip a byte, keep old signature)"
# Mutate the payload b64url; the old signature no longer authenticates it.
TPAY="$(echo "$PAYLOAD" | tr 'A-Za-z' 'N-ZA-Mn-za-m')"
C=$(code_for "${HDR}.${TPAY}.${SIG}")
[ "$C" = "401" ] && ok "tampered payload → 401" || bad "tampered payload → $C (expected 401)"

echo "→ garbage / non-JWT token"
C=$(code_for "not.a.real.jwt")
[ "$C" = "401" ] && ok "garbage token → 401" || bad "garbage → $C (expected 401)"

echo "→ no Authorization header"
C=$(code_for "")
[ "$C" = "401" ] && ok "absent bearer → 401" || bad "absent bearer → $C (expected 401)"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
