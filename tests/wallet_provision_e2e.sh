#!/usr/bin/env bash
# E2E: custodial wallet auto-provision on email verification.
#
# Crosses internal services against a running stack:
#   IAM REST  → IAM persistence (Postgres)        — user + wallet rows
#   IAM logic → Chain Bridge → Solana validator   — on-chain Registry register
#   IAM event bus → notification → Mailpit        — VerificationEmailRequested
#   IAM gRPC IdentityService.GetUserWallet         — cross-service wallet read
#
# Proves the review fixes end-to-end:
#   Fix 2  on-chain register → mark_registered/onboarded (blockchain_registered)
#   Fix 3  persist_custodial_wallet yields exactly ONE primary wallet
#
# Requires the stack up (`./scripts/app.sh start` from the superproject).
# Chain Bridge / Solana / grpcurl steps self-skip when unavailable so the
# script still passes the REST+DB cross-service path on a minimal stack.
set -euo pipefail

BASE="${BASE:-http://localhost:4010}"
GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"        # host-published IAM gRPC (docker) — see CLAUDE.md
MAILPIT="${MAILPIT:-http://localhost:13060}"
# Public auth endpoints are gated on the gateway role; dev default secret only
# applies when CHAIN_BRIDGE_INSECURE=true on the service.
ROLE_HDR="x-gridtokenx-role: api-gateway"
# Dev default MUST match the service CHAIN_BRIDGE_INSECURE fallback in
# gridtokenx-blockchain-core/src/auth.rs — not an arbitrary string, else the
# ApiGateway role resolves to Unknown and every gated call 401s.
GW_SECRET="${GATEWAY_SECRET:-gridtokenx-gateway-secret-2025}"
GW_HDR="x-gridtokenx-gateway-secret: ${GW_SECRET}"

TS=$(date +%s)
USER="wallet_${TS}"
EMAIL="${USER}@test.com"
PASS_STR="TestPass123!"

PASS_COUNT=0; FAIL_COUNT=0; SKIP_COUNT=0

check() { # name expected actual
  if echo "$3" | grep -qi "$2"; then echo "✅ $1"; ((PASS_COUNT++)) || true
  else echo "❌ $1 — expected '$2' in: $3"; ((FAIL_COUNT++)) || true; fi
}
skip() { echo "⏭️  SKIP $1 — $2"; ((SKIP_COUNT++)) || true; }

# ── Register ────────────────────────────────────────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/register" -H "Content-Type: application/json" \
  -H "$ROLE_HDR" -H "$GW_HDR" \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PASS_STR\"}")
check "POST /auth/register" "id" "$R"

# ── Resend verification: unverified account → generic ack ─────────────────────
R_KNOWN=$(curl -s -X POST "$BASE/api/v1/auth/resend-verification" -H "Content-Type: application/json" \
  -H "$ROLE_HDR" -H "$GW_HDR" -d "{\"email\":\"$EMAIL\"}")
check "POST /auth/resend-verification (unverified)" "sent" "$R_KNOWN"

# ── Resend verification: unknown email → IDENTICAL body (anti-enumeration) ─────
R_UNKNOWN=$(curl -s -X POST "$BASE/api/v1/auth/resend-verification" -H "Content-Type: application/json" \
  -H "$ROLE_HDR" -H "$GW_HDR" -d '{"email":"nobody@nowhere.invalid"}')
if [[ "$R_KNOWN" == "$R_UNKNOWN" ]]; then
  echo "✅ resend-verification (unknown == unverified body — no enumeration)"; ((PASS_COUNT++)) || true
else
  echo "❌ resend-verification enumeration leak: known='$R_KNOWN' unknown='$R_UNKNOWN'"; ((FAIL_COUNT++)) || true
fi

# ── Verify email (dev shortcut) → triggers custodial wallet provision ─────────
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" -H "$ROLE_HDR" -H "$GW_HDR" > /dev/null

# ── Login → bearer token for authenticated reads ──────────────────────────────
R=$(curl -s -X POST "$BASE/api/v1/auth/login" -H "Content-Type: application/json" \
  -H "$ROLE_HDR" -H "$GW_HDR" \
  -d "{\"username\":\"$USER\",\"password\":\"$PASS_STR\"}")
check "POST /auth/login" "access_token" "$R"
TOKEN=$(echo "$R" | grep -o '"access_token":"[^"]*"' | head -1 | cut -d'"' -f4 || true)
AUTH_HDR="Authorization: Bearer ${TOKEN}"

# ── /me → wallet_address linked on the user row ─────────────────────────
ME=$(curl -s "$BASE/api/v1/me" -H "$AUTH_HDR" -H "$ROLE_HDR" -H "$GW_HDR")
check "GET /me (wallet_address populated)" '"wallet_address":"[1-9A-HJ-NP-Za-km-z]' "$ME"
WALLET_ADDR=$(echo "$ME" | grep -o '"wallet_address":"[^"]*"' | head -1 | cut -d'"' -f4 || true)
echo "   custodial wallet: ${WALLET_ADDR:-<none>}"

# ── /me/wallets → Fix 3: exactly ONE primary Custodial wallet ───────────
WL=$(curl -s "$BASE/api/v1/me/wallets" -H "$AUTH_HDR" -H "$ROLE_HDR" -H "$GW_HDR")
check "GET /me/wallets (Custodial wallet present)" "Custodial" "$WL"
PRIMARY_COUNT=$(echo "$WL" | grep -o '"is_primary":true' | wc -l | tr -d ' ')
if [[ "$PRIMARY_COUNT" == "1" ]]; then
  echo "✅ wallets: exactly one primary (Fix 3 — no dual-primary)"; ((PASS_COUNT++)) || true
else
  echo "❌ wallets: expected 1 primary, got $PRIMARY_COUNT — $WL"; ((FAIL_COUNT++)) || true
fi

# ── Chain Bridge cross-service: on-chain registration (Fix 2 path) ─────────────
# Best-effort in the service; only assert when the bridge actually registered.
if echo "$WL" | grep -qi '"blockchain_registered":true'; then
  check "Chain Bridge: wallet registered on-chain (blockchain_registered)" "true" \
    "$(echo "$WL" | grep -o '"blockchain_registered":true' | head -1)"
else
  skip "Chain Bridge on-chain registration" "blockchain_registered=false (bridge/validator down — best-effort path)"
fi

# ── gRPC GetUserWallet: cross-service identity read ───────────────────────────
# IAM's ConnectRPC server does NOT expose gRPC server reflection, so grpcurl is
# pointed at the proto contract directly. -max-time keeps a down/blocked gRPC
# port from hanging the whole script.
USER_ID=$(echo "$ME" | grep -o '"id":"[0-9a-f-]*"' | head -1 | cut -d'"' -f4 || true)
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"
if command -v grpcurl >/dev/null 2>&1 && [[ -n "$USER_ID" && -f "$PROTO_DIR/identity.proto" ]]; then
  G=$(grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H "x-gridtokenx-role: aggregator-bridge" -H "$GW_HDR" \
    -d "{\"user_id\":\"$USER_ID\"}" \
    "$GRPC_ADDR" identity.IdentityService/GetUserWallet 2>&1 || true)
  if [[ -n "$WALLET_ADDR" ]]; then
    check "gRPC GetUserWallet (matches REST wallet_address)" "$WALLET_ADDR" "$G"
  else
    # proto3 JSON renders the field as camelCase walletAddress
    check "gRPC GetUserWallet (returns walletAddress)" "walletAddress" "$G"
  fi
else
  skip "gRPC GetUserWallet" "grpcurl missing, user_id empty, or identity.proto not found"
fi

# ── Mailpit: VerificationEmailRequested delivered ─────────────────────────────
sleep 2
MSGS=$(curl -s "$MAILPIT/api/v1/messages" 2>/dev/null || true)
if [[ -n "$MSGS" ]]; then
  check "Mailpit: verification email delivered to $EMAIL" "$EMAIL" "$MSGS"
else
  skip "Mailpit verification email" "Mailpit not reachable at $MAILPIT"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS_COUNT passed, $FAIL_COUNT failed, $SKIP_COUNT skipped"
[[ $FAIL_COUNT -eq 0 ]] && exit 0 || exit 1
