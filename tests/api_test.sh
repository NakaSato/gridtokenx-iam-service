#!/usr/bin/env bash
# IAM Service - Full API Endpoint Test
BASE="http://localhost:4010"
GW_ROLE="x-gridtokenx-role: api-gateway"
GW_SECRET="x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025"
TS=$(date +%s)
USER="testapi_${TS}"
EMAIL="${USER}@test.com"
PASS="TestPass123!"
PASS_WRONG="WrongPass999!"
WALLET="So1ana$(openssl rand -hex 16)"

PASS=0; FAIL=0

check() {
  local name="$1" expected="$2" actual="$3"
  if [[ "$actual" == *"$expected"* ]]; then
    echo "✅ $name"
    ((PASS++))
  else
    echo "❌ $name — expected '$expected' in: $actual"
    ((FAIL++))
  fi
}

# ── Health ──────────────────────────────────────────────────────────────────
R=$(curl -s "$BASE/health")
check "GET /health" "ok" "$R"

R=$(curl -s "$BASE/health/ready")
check "GET /health/ready" "ok" "$R"

R=$(curl -s "$BASE/health/live")
check "GET /health/live" "ok" "$R"

R=$(curl -s "$BASE/metrics")
check "GET /metrics" "http_requests" "$R"

# ── Register ─────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS\"}")
check "POST /auth/register (new user)" "user_id" "$R"

R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS\"}")
check "POST /auth/register (duplicate)" "409\|already\|conflict\|exists" "$(echo $R | tr '[:upper:]' '[:lower:]')"

R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d '{"username":"x","email":"bad","password":"short"}')
check "POST /auth/register (invalid input)" "400\|invalid\|error\|validation" "$(echo $R | tr '[:upper:]' '[:lower:]')"

# ── Activate user in TEST_MODE ────────────────────────────────────────────────
docker exec gridtokenx-postgres psql -U gridtokenx_user -d gridtokenx -c \
  "UPDATE users SET is_active=true, email_verified=true WHERE username='$USER';" > /dev/null 2>&1

# ── Login ─────────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS\"}")
check "POST /auth/login (valid)" "access_token" "$R"
TOKEN=$(echo "$R" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)

R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_WRONG\"}")
check "POST /auth/login (wrong password)" "401\|invalid\|unauthorized\|incorrect" "$(echo $R | tr '[:upper:]' '[:lower:]')"

R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"nonexistent_user_xyz","password":"anything"}')
check "POST /auth/login (unknown user)" "401\|404\|invalid\|not found" "$(echo $R | tr '[:upper:]' '[:lower:]')"

# ── Verify email ──────────────────────────────────────────────────────────────
VERIFY_TOKEN=$(docker exec gridtokenx-postgres psql -U gridtokenx_user -d gridtokenx -t -c \
  "SELECT email_verification_token FROM users WHERE username='$USER';" 2>/dev/null | tr -d ' \n')

if [[ -n "$VERIFY_TOKEN" && "$VERIFY_TOKEN" != "null" ]]; then
  R=$(curl -s "$BASE/api/v1/auth/verify?token=$VERIFY_TOKEN")
  check "GET /auth/verify (valid token)" "verified\|success\|true" "$(echo $R | tr '[:upper:]' '[:lower:]')"
else
  echo "⏭  GET /auth/verify — skipped (no token, already verified)"
  ((PASS++))
fi

R=$(curl -s "$BASE/api/v1/auth/verify?token=invalid-token-xyz")
check "GET /auth/verify (invalid token)" "400\|401\|404\|invalid\|error" "$(echo $R | tr '[:upper:]' '[:lower:]')"

# ── /users/me ─────────────────────────────────────────────────────────────────
R=$(curl -s "$BASE/api/v1/users/me" \
  -H "$GW_ROLE" -H "$GW_SECRET" \
  -H "Authorization: Bearer $TOKEN")
check "GET /users/me (valid)" "\"username\"" "$R"

R=$(curl -s "$BASE/api/v1/users/me" \
  -H "$GW_ROLE" -H "$GW_SECRET")
check "GET /users/me (no token)" "401\|unauthorized\|missing" "$(echo $R | tr '[:upper:]' '[:lower:]')"

R=$(curl -s "$BASE/api/v1/users/me" \
  -H "Authorization: Bearer $TOKEN")
check "GET /users/me (no gateway headers)" "401\|403\|unauthorized\|forbidden" "$(echo $R | tr '[:upper:]' '[:lower:]')"

# ── Identity: onboard ─────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/identity/onboard" \
  -H "$GW_ROLE" -H "$GW_SECRET" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_type":"Prosumer","lat_e7":137000000,"long_e7":100000000}')
check "POST /identity/onboard (valid)" "success\|transaction\|message" "$(echo $R | tr '[:upper:]' '[:lower:]')"

R=$(curl -s -X POST "$BASE/api/v1/identity/onboard" \
  -H "Content-Type: application/json" \
  -d '{"user_type":"Prosumer","lat_e7":137000000,"long_e7":100000000}')
check "POST /identity/onboard (no auth)" "401\|403\|unauthorized" "$(echo $R | tr '[:upper:]' '[:lower:]')"

# ── Identity: link wallet ─────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/identity/wallets" \
  -H "$GW_ROLE" -H "$GW_SECRET" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"label\":\"test\",\"is_primary\":true}")
check "POST /identity/wallets (valid)" "wallet\|message\|address" "$(echo $R | tr '[:upper:]' '[:lower:]')"

R=$(curl -s -X POST "$BASE/api/v1/identity/wallets" \
  -H "$GW_ROLE" -H "$GW_SECRET" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"label\":\"test\",\"is_primary\":true}")
check "POST /identity/wallets (duplicate)" "409\|conflict\|already\|duplicate" "$(echo $R | tr '[:upper:]' '[:lower:]')"

R=$(curl -s -X POST "$BASE/api/v1/identity/wallets" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"is_primary\":false}")
check "POST /identity/wallets (no auth)" "401\|403\|unauthorized" "$(echo $R | tr '[:upper:]' '[:lower:]')"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed out of $((PASS+FAIL)) tests"
[[ $FAIL -eq 0 ]] && exit 0 || exit 1
