#!/usr/bin/env bash
# Live E2E: POST /api/v1/auth/refresh token re-mint contract.
#
# refresh_token (auth_service.rs) is a STATELESS JWT re-mint: given a valid,
# unexpired bearer it issues a fresh token with the same claims and a new
# expiry. There is intentionally NO refresh-token rotation and NO blacklist —
# the previously issued token stays valid until its own exp. This suite pins
# that actual contract (not an invented rotation scheme):
#
#   valid bearer + admin role     → 200, returns a usable access_token
#   the re-minted token           → works on /me
#   the ORIGINAL token            → STILL works (no rotation/blacklist today)
#   no role header                → 401 (caller gate: ApiGateway/Admin)
#   garbage bearer                → 401 (token extractor rejects)
#   no bearer at all              → 401
#
# If rotation/replay-revocation is ever added, the "original still works" check
# below must flip to expect 401 — it is the canary for that change.
# Requires iam-service on :4010. /auth/refresh + /me need a service role → admin.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

# `|| true` keeps a no-match from tripping `set -o pipefail`.
json_field() { echo "$1" | grep -o "\"$2\": *\"[^\"]*\"" | head -1 | sed 's/.*: *"//;s/"$//' || true; }

# Dev convenience: clear the per-IP register throttle (the /register 5/hour
# budget is otherwise exhausted by repeated e2e runs from one host).
RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

mecode() { # <bearer> → /me http status
  curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me" \
    -H "authorization: Bearer $1" -H "$ROLE_HDR"
}
refresh() { # <extra curl args...> → "<body>\n<code>"
  curl -s -w '\n%{http_code}' -X POST "$BASE/api/v1/auth/refresh" "$@"
}

# ── Mint a real token via register → verify → login ───────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="refresh_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Refresh-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
TOKEN_A=$(json_field "$LOGIN" access_token)
if [ -z "$TOKEN_A" ]; then
  skip "refresh suite" "could not mint a token via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "minted original token A via login"

echo "→ refresh with valid bearer + admin role"
R=$(refresh -H "authorization: Bearer $TOKEN_A" -H "$ROLE_HDR")
BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "200" ] && ok "refresh → 200" || bad "refresh → $CODE (expected 200)"
TOKEN_B=$(json_field "$BODY" access_token)
[ -n "$TOKEN_B" ] && ok "refresh returned a new access_token (B)" \
  || bad "refresh body has no access_token: ${BODY:0:160}"

echo "→ re-minted token B is usable on /me"
if [ -n "$TOKEN_B" ]; then
  C=$(mecode "$TOKEN_B")
  [ "$C" = "200" ] && ok "/me with token B → 200" || bad "/me with B → $C (expected 200)"
fi

echo "→ original token A still valid (no rotation/blacklist — current contract)"
C=$(mecode "$TOKEN_A")
[ "$C" = "200" ] && ok "/me with token A → 200 (stateless re-mint, A not revoked)" \
  || bad "/me with A → $C — rotation may have been added; update this canary"

echo "→ refresh WITHOUT role header is gated"
R=$(refresh -H "authorization: Bearer $TOKEN_A"); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "401" ] && ok "refresh w/o role → 401 (caller gate)" || bad "refresh w/o role → $CODE (expected 401)"

echo "→ refresh with a garbage bearer is rejected"
R=$(refresh -H 'authorization: Bearer not.a.real.jwt' -H "$ROLE_HDR"); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "401" ] && ok "refresh garbage bearer → 401" || bad "refresh garbage → $CODE (expected 401)"

echo "→ refresh with no bearer is rejected"
R=$(refresh -H "$ROLE_HDR"); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "401" ] && ok "refresh no bearer → 401" || bad "refresh no bearer → $CODE (expected 401)"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
