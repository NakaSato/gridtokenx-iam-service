#!/usr/bin/env bash
# Comprehensive IAM Service API Explorer
#
# This script calls every API endpoint of the IAM service and prints responses.
# It also registers users with different types (Prosumer, Consumer).
#
# Usage: ./tests/register_users_by_type.sh [BASE_URL]

set -euo pipefail

BASE="${BASE:-${1:-http://localhost:4010}}"
GW_HEADERS=(
  -H "x-gridtokenx-role: api-gateway"
  -H "x-gridtokenx-gateway-secret: gridtokenx-gateway-secret-2025"
)

TS=$(date +%s)

# Helper: Print section header
header() {
  echo -e "\n\033[1;35m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m"
  echo -e "\033[1;34m  ▶ $1\033[0m"
  echo -e "\033[1;35m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m"
}

# Helper: Call API and print response
call_api() {
  local method="$1"
  local path="$2"
  shift 2
  
  local current_base="$BASE"
  local extras=()
  
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == "--url-override" ]]; then
      current_base="$2"
      shift 2
    else
      extras+=("$1")
      shift
    fi
  done
  
  echo -e "\n\033[1;32m[ $method $path ]\033[0m" >&2
  
  local resp
  resp=$(curl -s -X "$method" "$current_base$path" ${extras:+"${extras[@]}"})
  
  if echo "$resp" | jq . >/dev/null 2>&1; then
    echo "$resp" | jq . >&2
  else
    echo "$resp" | head -n 20 >&2
  fi
  
  echo "$resp"
}

# ── System Endpoints ──────────────────────────────────────────────────────────
header "SYSTEM ENDPOINTS"
call_api GET "/health"
call_api GET "/health/ready"
call_api GET "/api/v1/system/config"
# call_api GET "/metrics" # Skipping as it's very long, but available

# ── User Flows ────────────────────────────────────────────────────────────────

register_and_explore() {
  local type="$1"
  local type_lower=$(echo "$type" | tr '[:upper:]' '[:lower:]')
  local username="user_${type_lower}_${TS}"
  local email="${username}@example.com"
  local pass="TestPass123!"
  
  header "EXPLORING $type ($username)"

  # 1. Register
  RESP=$(call_api POST "/api/v1/auth/register" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"$username\",\"email\":\"$email\",\"password\":\"$pass\"}")
  
  if [[ "$RESP" != *"id"* ]]; then exit 1; fi

  # 2. Verify
  call_api GET "/api/v1/auth/verify?token=verify_$email"

  # 3. Login
  LOGIN_RESP=$(call_api POST "/api/v1/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"$username\",\"password\":\"$pass\"}")
  
  TOKEN=$(echo "$LOGIN_RESP" | jq -r '.access_token' || true)
  if [[ "$TOKEN" == "null" || -z "$TOKEN" ]]; then echo "❌ Login failed"; exit 1; fi

  AUTH_HEADERS=(-H "Authorization: Bearer $TOKEN")

  # 4. Profile (/me)
  call_api GET "/api/v1/users/me" "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}"

  # 5. Identity: Link Wallet
  # Generate a fresh wallet
  solana-keygen new --no-bip39-passphrase --silent --outfile "/tmp/w_$TS.json" > /dev/null 2>&1
  WALLET=$(solana-keygen pubkey "/tmp/w_$TS.json")
  rm "/tmp/w_$TS.json"

  RESP=$(call_api POST "/api/v1/users/me/wallets" \
    "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}" \
    -H "Content-Type: application/json" \
    -d "{\"wallet_address\":\"$WALLET\",\"label\":\"Primary\",\"is_primary\":true}")
  
  WALLET_ID=$(echo "$RESP" | jq -r '.id' || true)

  # 6. Identity: List Wallets
  call_api GET "/api/v1/users/me/wallets" "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}"

  if [[ "$WALLET_ID" != "null" && -n "$WALLET_ID" ]]; then
    # 7. Identity: Get Single Wallet
    call_api GET "/api/v1/users/me/wallets/$WALLET_ID" "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}"
    
    # 8. Identity: Set Primary (redundant but calls the endpoint)
    call_api PUT "/api/v1/users/me/wallets/$WALLET_ID/primary" "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}"
  fi

  # 9. Onboard
  call_api POST "/api/v1/users/me/onchain-profile" \
    "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}" \
    -H "Content-Type: application/json" \
    -d "{\"user_type\":\"$type_lower\",\"location\":{\"lat_e7\":13750000,\"long_e7\":100500000}}"

  # 10. Link Second Wallet for Delete Test
  solana-keygen new --no-bip39-passphrase --silent --outfile "/tmp/w2_$TS.json" > /dev/null 2>&1
  WALLET2=$(solana-keygen pubkey "/tmp/w2_$TS.json")
  rm "/tmp/w2_$TS.json"

  RESP2=$(call_api POST "/api/v1/users/me/wallets" \
    "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}" \
    -H "Content-Type: application/json" \
    -d "{\"wallet_address\":\"$WALLET2\",\"label\":\"Secondary\",\"is_primary\":false}")
  
  WALLET2_ID=$(echo "$RESP2" | jq -r '.id' || true)
  
  if [[ "$WALLET2_ID" != "null" && -n "$WALLET2_ID" ]]; then
    # 11. Identity: Delete Wallet
    call_api DELETE "/api/v1/users/me/wallets/$WALLET2_ID" "${GW_HEADERS[@]}" "${AUTH_HEADERS[@]}"
  fi
}

# Run for Prosumer
register_and_explore "Prosumer"

# Run for Consumer
register_and_explore "Consumer"

# ── Password Flow ─────────────────────────────────────────────────────────────
header "PASSWORD RESET FLOW"
USER_PW="pw_${TS}"
EMAIL_PW="${USER_PW}@example.com"

# Register for PW reset
call_api POST "/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER_PW\",\"email\":\"$EMAIL_PW\",\"password\":\"TestPass123!\"}" > /dev/null

call_api POST "/api/v1/auth/forgot-password" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"$EMAIL_PW\"}"

# Note: In TEST_MODE, verify token is verify_EMAIL. 
# For reset password, usually it's a random token sent to email.
# Since we are in TEST_MODE, let's see if there's a shortcut.
# Looking at iam-logic: reset_token is generated randomly.
# We'd need Mailpit to get it. 
header "TRADING SERVICE EXPLORATION"
TRADING_BASE="${TRADING_BASE:-http://localhost:8081}"

# 1. Market Stats
call_api GET "/api/v1/markets/stats" "" "" --url-override "$TRADING_BASE"

# 2. Order Book
call_api GET "/api/v1/markets/zones/1/order-book" "" "" --url-override "$TRADING_BASE"

# 3. Submit Order (Mock)
call_api POST "/api/v1/orders" \
  -H "Content-Type: application/json" \
  -d "{\"side\":\"buy\",\"order_type\":\"limit\",\"energy_amount_kwh\":\"100.50\",\"price_per_kwh\":\"4.50\",\"zone_id\":1}" \
  --url-override "$TRADING_BASE"

# 4. Quotes
call_api POST "/api/v1/quotes" \
  -H "Content-Type: application/json" \
  -d "{\"buyer_zone_id\":1,\"seller_zone_id\":2,\"energy_amount_kwh\":\"50.00\",\"agreed_price\":\"4.45\"}" \
  --url-override "$TRADING_BASE"

header "API EXPLORATION COMPLETE"
