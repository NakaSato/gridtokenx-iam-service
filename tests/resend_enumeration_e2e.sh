#!/usr/bin/env bash
# Live E2E: /auth/resend-verification anti-enumeration contract.
#
# resend_cooldown_e2e.sh proves the per-account cooldown; this proves the
# endpoint is not an account-enumeration oracle (auth_service resend_verification):
# the response MUST be byte-identical regardless of whether the email is
#
#   unknown (never registered)        → 200 {"status":"sent", <generic msg>}
#   registered + still UNVERIFIED     → 200 {"status":"sent", <same msg>}
#   registered + ALREADY verified     → 200 {"status":"sent", <same msg>}
#
# If any of the three differs (status, body, or HTTP code) the endpoint leaks
# whether an address exists / is verified. Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"

PASS=0; FAIL=0; SKIP=0
ok()  { echo "✅ $1"; PASS=$((PASS+1)); }
bad() { echo "❌ $1"; FAIL=$((FAIL+1)); }

resend() { # <email> → "<code>\t<body>"
  local r code body
  r=$(curl -s -w '\n%{http_code}' -X POST "$BASE/api/v1/auth/resend-verification" \
    -H 'content-type: application/json' -d "{\"email\":\"$1\"}")
  code=$(echo "$r" | tail -1); body=$(echo "$r" | sed '$d')
  printf '%s\t%s' "$code" "$body"
}

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

STAMP="$(date +%s)$RANDOM"
UNKNOWN="resend_unknown_${STAMP}@example.com"
UNVER="resend_unver_${STAMP}";  UNVER_E="${UNVER}@example.com"
VER="resend_ver_${STAMP}";      VER_E="${VER}@example.com"
PW='GridTokenX-$Resend-2025!'

# unverified account: register, do NOT verify
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$UNVER\",\"email\":\"$UNVER_E\",\"password\":\"$PW\"}" >/dev/null
# verified account: register + verify
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$VER\",\"email\":\"$VER_E\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$VER_E" >/dev/null

R_UNK="$(resend "$UNKNOWN")"
R_UNV="$(resend "$UNVER_E")"
R_VER="$(resend "$VER_E")"

C_UNK="${R_UNK%%$'\t'*}"; B_UNK="${R_UNK#*$'\t'}"
C_UNV="${R_UNV%%$'\t'*}"; B_UNV="${R_UNV#*$'\t'}"
C_VER="${R_VER%%$'\t'*}"; B_VER="${R_VER#*$'\t'}"

echo "→ all three return 200"
{ [ "$C_UNK" = "200" ] && [ "$C_UNV" = "200" ] && [ "$C_VER" = "200" ]; } \
  && ok "unknown/unverified/verified → 200/200/200" \
  || bad "codes differ: unknown=$C_UNK unverified=$C_UNV verified=$C_VER"

echo "→ all three carry status:sent"
echo "$B_UNK" | grep -q '"status":"sent"' && echo "$B_UNV" | grep -q '"status":"sent"' \
  && echo "$B_VER" | grep -q '"status":"sent"' \
  && ok "all responses status=sent" \
  || bad "status field not uniformly 'sent': [$B_UNK] [$B_UNV] [$B_VER]"

echo "→ bodies are byte-identical (no enumeration oracle)"
if [ "$B_UNK" = "$B_UNV" ] && [ "$B_UNV" = "$B_VER" ]; then
  ok "identical body across unknown/unverified/verified"
else
  bad "bodies differ — leaks account state:"
  bad "  unknown   : ${B_UNK:0:140}"
  bad "  unverified: ${B_UNV:0:140}"
  bad "  verified  : ${B_VER:0:140}"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
