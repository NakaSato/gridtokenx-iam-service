#!/usr/bin/env bash
# IAM Service - Comprehensive API Test Suite (via APISIX Gateway)
#
# This script tests the IAM service endpoints:
# - Health & System (/health, /metrics)
# - Auth (Register, Verify, Login, Profile)
# - Identity (Wallet linking, listing, management)
# - Password Management (Forgot, Reset)
#
# Requires:
# - APISIX Gateway running on localhost:4001
# - Mailpit running on localhost:13060
# - Gateway secrets (for /me and /identity endpoints)

set -euo pipefail

# Default to APISIX Gateway port
BASE="${BASE:-http://localhost:4001}"
MAILPIT="${MAILPIT:-http://localhost:13060}"

GW_HEADERS=(
  -H "x-gridtokenx-role: api-gateway"
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025"
)

TS=$(date +%s)
USER="testuser_${TS}"
EMAIL="${USER}@test.com"
PASS="TestPass123!"
NEW_PASS="NewPass456!"

# Generate unique valid Solana wallets
if command -v solana-keygen &> /dev/null; then
  WALLET=$(solana-keygen new --no-passphrase --no-outfile | grep "pubkey:" | cut -d" " -f2)
  WALLET2=$(solana-keygen new --no-passphrase --no-outfile | grep "pubkey:" | cut -d" " -f2)
else
  WALLET="BT9ESAZoNGnvPswpeHNLgt582GTQrAUv21ZLkk4H6WqS"
  WALLET2="7WvD8p6uYp6X9X5y7Q5P6Y7y7Q5P6Y7y7Q5P6Y7y7Q5P"
fi

PASS_COUNT=0
FAIL_COUNT=0

# Helper: Print header
header() {
  echo -e "\n\033[1;34m=== $1 ===\033[0m"
}

# Helper: Check result
check() {
  local name="$1" expected="$2" actual="$3"
  # Use grep -F for literal match and handle large output via pipe
  if echo "$actual" | grep -Fqi "$expected"; then
    echo -e "✅ \033[0;32m$name\033[0m"
    ((PASS_COUNT++)) || true
  else
    echo -e "❌ \033[0;31m$name\033[0m"
    echo "   Expected '$expected' to be in response"
    ((FAIL_COUNT++)) || true
  fi
}

# ── Health & Metrics ──────────────────────────────────────────────────────────
header "System Endpoints"

R=$(curl -s "$BASE/health")
check "GET /health" "ok" "$R"

# Optional: /health/ready might not be mapped in APISIX
R=$(curl -s "$BASE/health/ready")
if echo "$R" | grep -qi "404 Route Not Found"; then
  echo "⚠️  GET /health/ready - Not mapped in Gateway (Skipping)"
else
  check "GET /health/ready" "ok" "$R"
fi

R=$(curl -s "$BASE/metrics")
check "GET /metrics" "iam_http_requests_total" "$R"

# ── Authentication Flow ───────────────────────────────────────────────────────
header "Authentication Flow"

# 1. Register
R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS\"}")
check "Register new user" "id" "$R"

# 2. Verify (TEST_MODE shortcut)
R=$(curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL")
check "Verify email (TEST_MODE)" "success" "$R"

# 3. Login
R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS\"}")
check "Login valid credentials" "access_token" "$R"

TOKEN=$(echo "$R" | grep -o '"access_token":"[^"]*"' | cut -d'"' -f4)

# 4. Profile (/me)
echo "Testing /me (Note: May fail if Gateway strips x-gridtokenx-role header)"
R=$(curl -s "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" "$BASE/api/v1/users/me")
check "Get profile (/me)" "\"username\":\"$USER\"" "$R"

# ── Identity & Wallet Management ──────────────────────────────────────────────
header "Identity & Wallets"

# 1. Link Wallet
R=$(curl -s -X POST "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"wallet_address\":\"$WALLET\",\"label\":\"Primary\",\"is_primary\":true}" \
  "$BASE/api/v1/users/me/wallets")
check "Link first wallet (primary)" "\"id\":" "$R"

if echo "$R" | grep -qi "\"id\":"; then
  WALLET_ID=$(echo "$R" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

  # 2. Onboard
  R=$(curl -s -X POST "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"user_type":"prosumer","location":{"lat_e7":13750000,"long_e7":100500000}}' \
    "$BASE/api/v1/users/me/onchain-profile")
  check "Onboard user" "status" "$R"

  # 3. List Wallets
  R=$(curl -s "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" "$BASE/api/v1/users/me/wallets")
  check "List wallets" "\"wallet_address\":\"$WALLET\"" "$R"

  # 4. Get Single Wallet
  R=$(curl -s "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" "$BASE/api/v1/users/me/wallets/$WALLET_ID")
  check "Get wallet details" "$WALLET_ID" "$R"

  # 5. Link second wallet
  R=$(curl -s -X POST "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"wallet_address\":\"$WALLET2\",\"label\":\"Secondary\",\"is_primary\":false}" \
    "$BASE/api/v1/users/me/wallets")
  check "Link second wallet" "\"id\":" "$R"
  
  if echo "$R" | grep -qi "\"id\":"; then
    WALLET2_ID=$(echo "$R" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

    # 6. Set secondary as primary
    R=$(curl -s -X PUT "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" \
      "$BASE/api/v1/users/me/wallets/$WALLET2_ID/primary")
    check "Set wallet2 as primary" "\"is_primary\":true" "$R"

    # 7. Unlink (Delete) wallet1 (which is no longer primary)
    R=$(curl -s -X DELETE "${GW_HEADERS[@]}" -H "Authorization: Bearer $TOKEN" \
      "$BASE/api/v1/users/me/wallets/$WALLET_ID")
    check "Unlink wallet1" "success" "$R"
  fi
else
  echo "⚠️  Skipping identity tests because wallet linking failed (likely due to Gateway header stripping)"
fi

# ── Password Management ───────────────────────────────────────────────────────
header "Password Management"

# 1. Forgot Password
R=$(curl -s -X POST "$BASE/api/v1/auth/forgot-password" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"$EMAIL\"}")
check "Forgot password request" "sent" "$R"

# 2. Reset Password (via Mailpit)
echo "⏳ Checking Mailpit for reset token..."
sleep 2
MESSAGES=$(curl -s "$MAILPIT/api/v1/messages" || echo "FAILED")

if [[ "$MESSAGES" == "FAILED" ]]; then
  echo "⚠️  Mailpit not available at $MAILPIT. Skipping reset password test."
else
  MAILPIT_ID=$(echo "$MESSAGES" | grep -o '"ID":"[^"]*"' | head -1 | cut -d'"' -f4 || true)
  if [[ -n "$MAILPIT_ID" ]]; then
    MSG_SOURCE=$(curl -s "$MAILPIT/api/v1/message/$MAILPIT_ID")
    RESET_TOKEN=$(echo "$MSG_SOURCE" | grep -o 'token=[a-f0-9-]*' | head -1 | cut -d= -f2 || true)
    
    if [[ -n "$RESET_TOKEN" ]]; then
      echo "🔑 Found token: $RESET_TOKEN"
      R=$(curl -s -X POST "$BASE/api/v1/auth/reset-password" \
        -H "Content-Type: application/json" \
        -d "{\"token\":\"$RESET_TOKEN\",\"new_password\":\"$NEW_PASS\"}")
      check "Reset password with token" "success" "$R"
      
      # Verify login with new password
      R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"username\":\"$USER\",\"password\":\"$NEW_PASS\"}")
      check "Login with new password" "access_token" "$R"
    else
      echo "❌ Could not find reset token in email."
      ((FAIL_COUNT++)) || true
    fi
  else
    echo "⚠️  No email found in Mailpit. Skipping reset."
  fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────
header "Test Summary"
echo -e "Total Passed: \033[0;32m$PASS_COUNT\033[0m"
echo -e "Total Failed: \033[0;31m$FAIL_COUNT\033[0m"

if [[ $FAIL_COUNT -eq 0 ]]; then
  echo -e "\n✨ \033[1;32mALL IAM SERVICE TESTS PASSED!\033[0m"
  exit 0
else
  echo -e "\n💥 \033[1;31mSOME TESTS FAILED!\033[0m"
  echo "Hint: If /me or identity tests failed with AUTH_1004, ensure APISIX is not stripping the 'x-gridtokenx-role' header."
  exit 1
fi
