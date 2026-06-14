#!/usr/bin/env bash
# Comprehensive curl test of all IAM REST endpoints (happy + error cases).
set -u
BASE="http://localhost:4010"
ADMIN=(-H 'x-gridtokenx-role: admin')   # passes RBAC on protected routes, no secret needed
TS=$(date +%s)
U="curltest_${TS}"
EMAIL="${U}@example.com"
PW='Gr1dT0kenX!safe'
PASS=0; FAIL=0
log(){ printf '\n=== %s ===\n' "$1"; }
# req NAME EXPECT METHOD PATH [curl-args...]
req(){
  local name="$1" expect="$2" method="$3" path="$4"; shift 4
  local out code body
  out=$(curl -s -m "${CURL_TIMEOUT:-10}" -w '\n%{http_code}' -X "$method" "$@" "${BASE}${path}")
  code=$(printf '%s' "$out" | tail -n1)
  body=$(printf '%s' "$out" | sed '$d')
  if [ "$code" = "$expect" ]; then PASS=$((PASS+1)); printf '✅ %-40s %s (want %s)\n' "$name" "$code" "$expect"
  else FAIL=$((FAIL+1)); printf '❌ %-40s %s (want %s)\n   body: %s\n' "$name" "$code" "$expect" "$(printf '%s' "$body" | head -c300)"; fi
  LAST_BODY="$body"
}

log "HEALTH / SYSTEM / METRICS"
req "health"          200 GET /health
req "health/live"     200 GET /health/live
req "health/ready"    200 GET /health/ready
req "metrics"         200 GET /metrics
req "system/config"   200 GET /api/v1/system/config

log "REGISTER"
req "register.happy"  200 POST /api/v1/auth/register -H 'content-type:application/json' \
  -d "{\"username\":\"$U\",\"email\":\"$EMAIL\",\"password\":\"$PW\",\"first_name\":\"Ct\",\"last_name\":\"Test\"}"
req "register.dup"    409 POST /api/v1/auth/register -H 'content-type:application/json' \
  -d "{\"username\":\"$U\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}"
req "register.missing-field" 422 POST /api/v1/auth/register -H 'content-type:application/json' \
  -d "{\"username\":\"x\"}"
req "register.bad-json" 400 POST /api/v1/auth/register -H 'content-type:application/json' -d "{not json"

log "VERIFY EMAIL (real token from DB) — required before login"
VTOKEN=$(docker exec gridtokenx-postgres psql -U gridtokenx_user -d gridtokenx -tAc \
  "SELECT email_verification_token FROM users WHERE username='$U'" 2>/dev/null | tr -d '[:space:]')
echo "   verify token: ${VTOKEN:0:16}..."
# verify triggers custodial wallet auto-provision via Chain Bridge; allow time for blockchain retries
CURL_TIMEOUT=25 req "verify.happy"    200 GET "/api/v1/auth/verify?token=$VTOKEN"

log "LOGIN"
req "login.happy"     200 POST /api/v1/auth/login -H 'content-type:application/json' \
  -d "{\"username\":\"$U\",\"password\":\"$PW\"}"
TOKEN=$(printf '%s' "$LAST_BODY" | sed -n 's/.*"access_token":"\([^"]*\)".*/\1/p')
echo "   token: ${TOKEN:0:24}..."
req "login.badpw"     401 POST /api/v1/auth/login -H 'content-type:application/json' \
  -d "{\"username\":\"$U\",\"password\":\"wrongpw\"}"
req "login.nouser"    401 POST /api/v1/auth/login -H 'content-type:application/json' \
  -d "{\"username\":\"ghost_$TS\",\"password\":\"$PW\"}"
req "login.missing"   422 POST /api/v1/auth/login -H 'content-type:application/json' -d "{\"username\":\"x\"}"

log "VERIFY / RESEND / PASSWORD RESET"
req "verify.badtoken" 400 GET "/api/v1/auth/verify?token=deadbeef"
req "verify.notoken"  400 GET "/api/v1/auth/verify"
req "resend.happy"    200 POST /api/v1/auth/resend-verification -H 'content-type:application/json' -d "{\"email\":\"$EMAIL\"}"
req "forgot.happy"    200 POST /api/v1/auth/forgot-password -H 'content-type:application/json' -d "{\"email\":\"$EMAIL\"}"
req "reset.badtoken"  400 POST /api/v1/auth/reset-password -H 'content-type:application/json' -d "{\"token\":\"bad\",\"new_password\":\"$PW\"}"

log "PROTECTED — no auth"
req "me.noauth"       401 GET /api/v1/me "${ADMIN[@]}"
req "wallets.noauth"  401 GET /api/v1/me/wallets "${ADMIN[@]}"

log "PROTECTED — with token (role=admin)"
AUTH=(-H "Authorization: Bearer ${TOKEN}")
req "me.ok"           200 GET /api/v1/me "${ADMIN[@]}" "${AUTH[@]}"
req "refresh.ok"      200 POST /api/v1/auth/refresh "${ADMIN[@]}" "${AUTH[@]}"
req "wallets.list"    200 GET /api/v1/me/wallets "${ADMIN[@]}" "${AUTH[@]}"
# Valid, unique 32-byte Solana pubkey per run (base58). wallet_address is globally unique in DB.
genpubkey(){ python3 -c '
import os
a="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
n=int.from_bytes(os.urandom(32),"big")
s=""
while n>0:
    n,r=divmod(n,58); s=a[r]+s
print(s.rjust(44,"1"))
'; }
WADDR=$(genpubkey)
req "wallet.link"     200 POST /api/v1/me/wallets "${ADMIN[@]}" "${AUTH[@]}" -H 'content-type:application/json' \
  -d "{\"wallet_address\":\"$WADDR\",\"label\":\"Primary\",\"is_primary\":true}"
WID=$(printf '%s' "$LAST_BODY" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')   # capture BEFORE dup overwrites LAST_BODY
echo "   wallet id: $WID"
req "wallet.link.dup" 409 POST /api/v1/me/wallets "${ADMIN[@]}" "${AUTH[@]}" -H 'content-type:application/json' \
  -d "{\"wallet_address\":\"$WADDR\",\"label\":\"Dup\",\"is_primary\":false}"
req "wallet.get"      200 GET "/api/v1/me/wallets/$WID" "${ADMIN[@]}" "${AUTH[@]}"
req "wallet.setprim"  200 PATCH "/api/v1/me/wallets/$WID" "${ADMIN[@]}" "${AUTH[@]}" -H 'content-type:application/json' -d '{"is_primary":true}'
req "wallet.get.badid" 400 GET "/api/v1/me/wallets/not-a-uuid" "${ADMIN[@]}" "${AUTH[@]}"
req "wallet.del.primary" 400 DELETE "/api/v1/me/wallets/$WID" "${ADMIN[@]}" "${AUTH[@]}"  # primary cannot be deleted (by design)
# link a 2nd, non-primary wallet then delete it
WADDR2=$(genpubkey)
req "wallet.link2"    200 POST /api/v1/me/wallets "${ADMIN[@]}" "${AUTH[@]}" -H 'content-type:application/json' \
  -d "{\"wallet_address\":\"$WADDR2\",\"label\":\"Secondary\",\"is_primary\":false}"
WID2=$(printf '%s' "$LAST_BODY" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
req "wallet.delete"   200 DELETE "/api/v1/me/wallets/$WID2" "${ADMIN[@]}" "${AUTH[@]}"

# NOTE: onboard returns HTTP 200 with {"status":"failed"} when Chain Bridge has no RPC (graceful, by design — not 5xx)
log "ONCHAIN (needs Solana validator; degrades to status:failed when Chain Bridge has no RPC)"
CURL_TIMEOUT=30 req "onboard" 200 POST /api/v1/me/registration "${ADMIN[@]}" "${AUTH[@]}" -H 'content-type:application/json' \
  -d '{"user_type":"prosumer","location":{"lat_e7":13750000,"long_e7":100500000}}'

printf '\n==== RESULT: PASS=%d FAIL=%d ====\n' "$PASS" "$FAIL"
echo "TEST_START_EPOCH=$TS"
