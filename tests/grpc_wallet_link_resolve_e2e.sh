#!/usr/bin/env bash
# Live E2E: IdentityService LinkWallet → GetUserWallet functional path.
#
# grpc_rbac_e2e.sh only proves the role gate; this proves the wallet RPCs do
# real work end-to-end (identity_grpc.rs:198 / :229):
#
#   LinkWallet(user_id, addr)     → LinkWalletResponse with a wallet_id
#   GetUserWallet(user_id)        → the user's PRIMARY on-chain address
#   InitializeUserWallet          → SKIPPED (needs a Solana validator / Chain
#                                   Bridge to actually fund + create the PDA)
#
# A fresh verified account already has one auto-provisioned custodial wallet as
# primary, so GetUserWallet resolves it; the linked secondary (is_primary:false)
# proves LinkWallet persists without disturbing the primary. user_id comes from
# REST /me. Requires the stack up + grpcurl (+ Chain Bridge for the custodial
# wallet — degrades to skip if none is provisioned).
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"
# A syntactically valid base58 Solana address (wrapped-SOL mint) for the link.
LINK_ADDR='So11111111111111111111111111111111111111112'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "LinkWallet/GetUserWallet suite" "grpcurl missing or identity.proto not at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

json_field()  { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
gjson_field() { echo "$1" | grep -o "\"$2\": *\"[^\"]*\"" | head -1 | sed 's/.*: *"//;s/"$//' || true; }

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Fresh verified account → JWT → user_id ────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="glw_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Glw-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "LinkWallet/GetUserWallet suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ME=$(curl -s "$BASE/api/v1/me" -H "authorization: Bearer $JWT" -H 'x-gridtokenx-role: admin')
USER_ID=$(json_field "$ME" id)
if [ -z "$USER_ID" ]; then
  skip "LinkWallet/GetUserWallet suite" "could not resolve user_id from /me: ${ME:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "fresh account ready (user_id=$USER_ID)"

gcall() { # <method> <json-data>
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H 'x-gridtokenx-role: admin' -d "$2" \
    "$GRPC_ADDR" "identity.IdentityService/$1" 2>&1 || true
}

echo "→ LinkWallet (secondary, is_primary:false)"
R=$(gcall LinkWallet "{\"user_id\":\"$USER_ID\",\"wallet_address\":\"$LINK_ADDR\",\"label\":\"e2e-link\",\"is_primary\":false}")
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "LinkWallet wrongly hit RBAC denial: ${R:0:160}"
elif echo "$R" | grep -qE '"walletId"|"wallet_id"'; then
  ok "LinkWallet → wallet_id returned"
  ADDR=$(gjson_field "$R" walletAddress)
  [ "$ADDR" = "$LINK_ADDR" ] && ok "echoed wallet_address=$LINK_ADDR" \
    || bad "wallet_address=$ADDR (expected $LINK_ADDR)"
else
  bad "LinkWallet → no wallet_id: ${R:0:200}"
fi

echo "→ GetUserWallet resolves the primary address"
R=$(gcall GetUserWallet "{\"user_id\":\"$USER_ID\"}")
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "GetUserWallet wrongly hit RBAC denial: ${R:0:160}"
elif echo "$R" | grep -qiE 'notfound|not found'; then
  skip "GetUserWallet resolve" "no primary wallet (Chain Bridge down → no custodial provisioned)"
elif echo "$R" | grep -qE '"walletAddress"|"wallet_address"'; then
  WA=$(gjson_field "$R" walletAddress)
  [ -n "$WA" ] && ok "GetUserWallet → primary address ($WA)" \
    || bad "GetUserWallet returned an empty wallet_address: ${R:0:160}"
else
  bad "GetUserWallet → unexpected response: ${R:0:200}"
fi

skip "InitializeUserWallet" "needs a Solana validator + Chain Bridge to fund + create the PDA"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
