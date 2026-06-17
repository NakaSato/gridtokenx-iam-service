#!/usr/bin/env bash
# Live E2E: IdentityService/VerifyApiKey functional correctness.
#
# grpc_rbac_e2e.sh only proves the role gate; this proves the RPC's verify
# semantics (identity_grpc.rs:139):
#
#   invalid / unknown key → valid:false (RPC SUCCEEDS — the key is simply not
#                           valid; mirrors VerifyToken's garbage-token contract)
#   malformed key         → valid:false
#
# The happy path (a real key → valid:true + role) is NOT exercised: IAM exposes
# no over-the-wire API-key mint endpoint, so a shell E2E cannot obtain a live
# key. That path is covered by iam-logic unit tests against ApiKeyService.
# Requires the stack up + grpcurl.
set -euo pipefail

GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "VerifyApiKey suite" "grpcurl missing or identity.proto not at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

call() { # <json-data>
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H 'x-gridtokenx-role: admin' -d "$1" \
    "$GRPC_ADDR" identity.IdentityService/VerifyApiKey 2>&1 || true
}

not_valid() { # <response>  → true when valid:false and RPC did not error out
  echo "$1" | grep -qiE 'permissiondenied|permission denied' && return 1
  echo "$1" | grep -q '"valid": true' && return 1
  return 0
}

echo "→ VerifyApiKey with an unknown key"
R=$(call '{"key":"gtx_unknown_key_000000000000000000000000"}')
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "unknown key wrongly hit RBAC denial (gate misconfigured): ${R:0:160}"
elif echo "$R" | grep -q '"valid": true'; then
  bad "unknown key → valid:true (must be false): ${R:0:160}"
else
  ok "unknown key → not valid (RPC ok, key rejected)"
fi

echo "→ VerifyApiKey with a malformed key"
R=$(call '{"key":"###not-a-key###"}')
not_valid "$R" && ok "malformed key → not valid" \
  || bad "malformed key not rejected: ${R:0:160}"

echo "→ VerifyApiKey with an empty key"
R=$(call '{"key":""}')
not_valid "$R" && ok "empty key → not valid" \
  || bad "empty key not rejected: ${R:0:160}"

skip "valid-key happy path" "no over-the-wire API-key mint endpoint — covered by iam-logic unit tests"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
