#!/usr/bin/env bash
# Live E2E: gRPC IdentityService RBAC (fail-closed role gate).
#
# Covers the 7 IdentityService RPCs that wallet_provision_e2e.sh does NOT
# (it only exercises GetUserWallet). Asserts the role gate in
# gridtokenx-blockchain-core/src/auth.rs::ServiceRole::from_headers → require_any:
#
#   • missing x-gridtokenx-role            → permission_denied (fail-closed)
#   • unknown role                         → permission_denied
#   • valid-but-disallowed role per method → permission_denied
#   • allowed role                         → NOT permission_denied
#     (a downstream "invalid token"/"not found" still proves RBAC let it through)
#
# Allowlists (CLAUDE.md / auth.rs):
#   VerifyToken            = ApiGateway/TradingApi/AggregatorBridge/MeterService/Admin
#   VerifyApiKey           = ApiGateway/AggregatorBridge/Admin
#   GetUserInfo/Authorize/
#   RegisterUser/LinkWallet/
#   InitializeUserWallet   = ApiGateway/Admin
#   ApiGateway also requires x-gridtokenx-gateway-secret.
#
# IAM's ConnectRPC server exposes NO gRPC reflection → grpcurl is pointed at the
# proto contract directly. Requires the stack up + grpcurl.
set -euo pipefail

GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"   # host-published IAM gRPC (docker) — see CLAUDE.md
GW_SECRET="${GATEWAY_SECRET:-gridtokenx-gateway-secret-2025}"
GW_HDR="x-gridtokenx-gateway-secret: ${GW_SECRET}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "gRPC RBAC suite" "grpcurl missing or identity.proto not found at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

# call <rpc> <json> <header...> — echoes grpcurl combined output (status in stderr→merged).
call() {
  local rpc="$1" data="$2"; shift 2
  local hdrs=(); local h
  for h in "$@"; do hdrs+=(-H "$h"); done
  # ${hdrs[@]+...} guards the empty-array expansion under `set -u` on bash 3.2 (macOS).
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    ${hdrs[@]+"${hdrs[@]}"} -d "$data" \
    "$GRPC_ADDR" "identity.IdentityService/$rpc" 2>&1 || true
}
denied() { echo "$1" | grep -qiE 'permissiondenied|permission denied|permission_denied'; }

# assert_denied <name> <output>
assert_denied() { if denied "$2"; then ok "$1 → permission_denied"; else bad "$1 — expected permission_denied, got: ${2:0:120}"; fi; }
# assert_passed_rbac <name> <output>  (RBAC let it through; downstream error allowed)
assert_passed_rbac() { if denied "$2"; then bad "$1 — unexpected permission_denied: ${2:0:120}"; else ok "$1 → RBAC allowed (downstream-only error ok)"; fi; }

echo "── fail-closed: missing role header (every RPC denies) ──"
for rpc_data in \
  "VerifyToken {\"token\":\"x\"}" \
  "GetUserInfo {\"token\":\"x\"}" \
  "Authorize {\"token\":\"x\",\"required_permission\":\"read\"}" \
  "VerifyApiKey {\"key\":\"x\"}" \
  "RegisterUser {}" \
  "LinkWallet {}" \
  "InitializeUserWallet {}" ; do
  rpc="${rpc_data%% *}"; data="${rpc_data#* }"
  assert_denied "$rpc (no role)" "$(call "$rpc" "$data")"
done

echo "── fail-closed: unknown role denies ──"
assert_denied "VerifyToken (role=bogus)" "$(call VerifyToken '{"token":"x"}' 'x-gridtokenx-role: bogus-role')"

echo "── valid-but-disallowed role per method ──"
# VerifyApiKey disallows TradingApi (only ApiGateway/AggregatorBridge/Admin).
assert_denied "VerifyApiKey (role=trading-api, disallowed)" \
  "$(call VerifyApiKey '{"key":"x"}' 'x-gridtokenx-role: trading-api')"
# Authorize/GetUserInfo/RegisterUser disallow TradingApi & AggregatorBridge (ApiGateway/Admin only).
assert_denied "Authorize (role=trading-api, disallowed)" \
  "$(call Authorize '{"token":"x","required_permission":"read"}' 'x-gridtokenx-role: trading-api')"
assert_denied "RegisterUser (role=aggregator-bridge, disallowed)" \
  "$(call RegisterUser '{}' 'x-gridtokenx-role: aggregator-bridge')"

echo "── allowed role passes the gate ──"
# Admin passes everywhere, no gateway secret needed.
assert_passed_rbac "VerifyToken (role=admin)" "$(call VerifyToken '{"token":"deadbeef"}' 'x-gridtokenx-role: admin')"
assert_passed_rbac "VerifyApiKey (role=aggregator-bridge)" "$(call VerifyApiKey '{"key":"deadbeef"}' 'x-gridtokenx-role: aggregator-bridge')"
assert_passed_rbac "GetUserInfo (role=admin)" "$(call GetUserInfo '{"token":"deadbeef"}' 'x-gridtokenx-role: admin')"
# ApiGateway requires the gateway secret too.
assert_passed_rbac "Authorize (role=api-gateway + secret)" \
  "$(call Authorize '{"token":"deadbeef","required_permission":"read"}' 'x-gridtokenx-role: api-gateway' "$GW_HDR")"

echo "── ApiGateway WITHOUT gateway-secret is denied ──"
assert_denied "VerifyToken (api-gateway, no secret)" \
  "$(call VerifyToken '{"token":"x"}' 'x-gridtokenx-role: api-gateway')"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
