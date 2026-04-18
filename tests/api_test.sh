#!/usr/bin/env bash
# IAM Service - Full API Endpoint Test
BASE="http://localhost:4010"
GW_ROLE="x-gridtokenx-role: api-gateway"
GW_SECRET="x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025"
TS=$(date +%s)
USER="testapi_${TS}"
EMAIL="${USER}@test.com"
PASS_STR="TestPass123!"
PASS_WRONG="WrongPass999!"
WALLET="BT9ESAZoNGnvPswpeHNLgt582GTQrAUv21ZLkk4H6WqS"

SUCCESS_COUNT=0; FAIL_COUNT=0

check() {
  local name="$1" expected="$2" actual="$3"
  if [[ "$(echo "$actual" | tr '[:upper:]' '[:lower:]')" == *"$(echo "$expected" | tr '[:upper:]' '[:lower:]')"* ]]; then
    echo "✅ $name"
    ((SUCCESS_COUNT++))
  else
    echo "❌ $name — expected '$expected' in: $actual"
    ((FAIL_COUNT++))
  fi
}

# ── Health ──────────────────────────────────────────────────────────────────
R=$(curl -s "$BASE/health")
check "GET /health" "ok" "$R"

R=$(curl -s "$BASE/health/ready")
check "GET /health/ready" "ok" "$R"

R=$(curl -s "$BASE/health/live")
check "GET /health/live" "alive" "$R"

R=$(curl -s "$BASE/metrics")
check "GET /metrics" "iam_http_requests_total" "$R"

# ── Register ─────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS_STR\"}")
check "POST /auth/register (new user)" "\"id\":" "$R"

R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS_STR\"}")
check "POST /auth/register (duplicate)" "conflict" "$R"

R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d '{"username":"x","email":"bad","password":"short"}')
check "POST /auth/register (invalid input)" "at least 8 characters" "$R"

# ── Activate user in TEST_MODE ────────────────────────────────────────────────
# Use the verify endpoint with EMAIL
R=$(curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL")
check "GET /auth/verify (TEST_MODE)" "success" "$R"

# ── Login ─────────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_STR\"}")
check "POST /auth/login (valid)" "access_token" "$R"
TOKEN=$(echo "$R" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)

R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_WRONG\"}")
check "POST /auth/login (wrong password)" "invalid" "$R"

R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"nonexistent_user_xyz","password":"anything"}')
check "POST /auth/login (unknown user)" "invalid" "$R"

# ── Verify email ──────────────────────────────────────────────────────────────
# Already done via TEST_MODE above, but let's test invalid token
R=$(curl -s "$BASE/api/v1/auth/verify?token=invalid-token-xyz")
check "GET /auth/verify (invalid token)" "invalid" "$R"

# ── /users/me ─────────────────────────────────────────────────────────────────
R=$(curl -s "$BASE/api/v1/users/me" \
  -H "x-gridtokenx-role: api-gateway" \
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025" \
  -H "Authorization: Bearer $TOKEN")
check "GET /users/me (valid)" "\"username\"" "$R"

R=$(curl -s "$BASE/api/v1/users/me" \
  -H "x-gridtokenx-role: api-gateway" \
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025")
check "GET /users/me (no token)" "auth" "$R"

R=$(curl -s "$BASE/api/v1/users/me" \
  -H "Authorization: Bearer $TOKEN")
check "GET /users/me (no gateway headers)" "auth" "$R"

# ── Identity: link wallet ─────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/identity/wallets" \
  -H "x-gridtokenx-role: api-gateway" \
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"label\":\"test\",\"is_primary\":true}")
check "POST /identity/wallets (valid)" "wallet" "$R"

# ── Identity: onboard ─────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/identity/onboard" \
  -H "x-gridtokenx-role: api-gateway" \
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_type":"Prosumer","lat_e7":137000000,"long_e7":100000000}')
check "POST /identity/onboard (valid)" "success" "$R"

R=$(curl -s -X POST "$BASE/api/v1/identity/onboard" \
  -H "Content-Type: application/json" \
  -d '{"user_type":"Prosumer","lat_e7":137000000,"long_e7":100000000}')
check "POST /identity/onboard (no auth)" "auth" "$R"

# ── Identity: wallet extras ───────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/identity/wallets" \
  -H "x-gridtokenx-role: api-gateway" \
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"label\":\"test\",\"is_primary\":true}")
# This might fail with DB_7002 if not handled, but we check for failure
check "POST /identity/wallets (duplicate)" "error" "$R"

R=$(curl -s -X POST "$BASE/api/v1/identity/wallets" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"is_primary\":false}")
check "POST /identity/wallets (no auth)" "auth" "$R"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: $SUCCESS_COUNT passed, $FAIL_COUNT failed out of $((SUCCESS_COUNT+FAIL_COUNT)) tests"
[[ $FAIL_COUNT -eq 0 ]] && exit 0 || exit 1
