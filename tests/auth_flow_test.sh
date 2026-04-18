#!/usr/bin/env bash
# Test: register, login, forgot-password, reset-password
set -euo pipefail

BASE="http://localhost:4010"
MAILPIT="http://localhost:13060"
TS=$(date +%s)
USER="testpwd_${TS}"
EMAIL="${USER}@test.com"
PASS_STR="TestPass123!"
NEW_PASS="NewPass456!"

PASS_COUNT=0; FAIL_COUNT=0

check() {
  local name="$1" expected="$2" actual="$3"
  if echo "$actual" | grep -qi "$expected"; then
    echo "✅ $name"
    ((PASS_COUNT++)) || true
  else
    echo "❌ $name — expected '$expected' in: $actual"
    ((FAIL_COUNT++)) || true
  fi
}

# ── Register ──────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS_STR\"}")
check "POST /auth/register" "id" "$R"

# Activate user via TEST_MODE token
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" > /dev/null

# ── Login ─────────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_STR\"}")
check "POST /auth/login (valid credentials)" "access_token" "$R"

# Testing wrong password. We use a separate curl to get the HTTP code.
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE/api/v1/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USER\",\"password\":\"wrongpassword\"}")
check "POST /auth/login (wrong password → 401)" "401" "$HTTP_CODE"

# ── Forgot password ───────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/forgot-password" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"$EMAIL\"}")
check "POST /auth/forgot-password (registered email)" "reset link\|sent" "$R"

R=$(curl -s -X POST "$BASE/api/v1/auth/forgot-password" \
  -H "Content-Type: application/json" \
  -d '{"email":"nobody@nowhere.invalid"}')
check "POST /auth/forgot-password (unknown email — no enumeration)" "sent\|reset link" "$R"

# ── Extract reset token from Mailpit ─────────────────────────────────────────
echo ""
echo "⏳ Waiting for reset email in Mailpit..."
sleep 3

# List messages and find the latest one for our user
MESSAGES=$(curl -s "$MAILPIT/api/v1/messages")
MAILPIT_MSG=$(echo "$MESSAGES" | grep -o '"ID":"[^"]*"' | head -1 | cut -d'"' -f4 || true)

if [[ -z "$MAILPIT_MSG" ]]; then
  echo "❌ No emails found in Mailpit — skipping reset-password test"
  FAIL_COUNT=$((FAIL_COUNT+1))
else
  echo "Found message ID: $MAILPIT_MSG"
  # Get message source
  MSG_SOURCE=$(curl -s "$MAILPIT/api/v1/message/$MAILPIT_MSG")
  # Extract token from the link in the message body
  RESET_TOKEN=$(echo "$MSG_SOURCE" | grep -o 'token=[a-f0-9-]*' | head -1 | cut -d= -f2 || true)

  if [[ -z "$RESET_TOKEN" ]]; then
    echo "❌ Could not extract reset token from email body. Source snippet:"
    echo "$MSG_SOURCE" | head -n 20
    FAIL_COUNT=$((FAIL_COUNT+1))
  else
    echo "🔑 Reset token: $RESET_TOKEN"

    # ── Reset password ──────────────────────────────────────────────────────
    R=$(curl -s -X POST "$BASE/api/v1/auth/reset-password" \
      -H "Content-Type: application/json" \
      -d "{\"token\":\"$RESET_TOKEN\",\"new_password\":\"$NEW_PASS\"}")
    check "POST /auth/reset-password (valid token)" "success\|reset successfully" "$R"

    # Login with new password
    R=$(curl -s -X POST "$BASE/api/v1/auth/login" \
      -H "Content-Type: application/json" \
      -d "{\"username\":\"$USER\",\"password\":\"$NEW_PASS\"}")
    check "POST /auth/login (after password reset)" "access_token" "$R"

    # Old password should fail
    HTTP_CODE_OLD=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE/api/v1/auth/login" \
      -H "Content-Type: application/json" \
      -d "{\"username\":\"$USER\",\"password\":\"$PASS_STR\"}")
    check "POST /auth/login (old password rejected after reset)" "401" "$HTTP_CODE_OLD"

    # Token reuse should fail
    R=$(curl -s -X POST "$BASE/api/v1/auth/reset-password" \
      -H "Content-Type: application/json" \
      -d "{\"token\":\"$RESET_TOKEN\",\"new_password\":\"AnotherPass789!\"}")
    check "POST /auth/reset-password (token reuse rejected)" "400\|invalid\|expired" "$R"
  fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS_COUNT passed, $FAIL_COUNT failed"
[[ $FAIL_COUNT -eq 0 ]] && exit 0 || exit 1
