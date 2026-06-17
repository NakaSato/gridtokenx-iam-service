#!/usr/bin/env bash
# Live E2E: IdentityService/RegisterUser functional correctness.
#
# grpc_rbac_e2e.sh only proves the role gate; this proves the RPC actually
# creates a user (identity_grpc.rs:169) — the gateway-side registration path:
#
#   fresh username   → RegisterUserResponse echoing user_id + username + email
#   duplicate        → Internal error (unique constraint; not a second account)
#
# Side effect: it really inserts a row. Names are unique-stamped so reruns don't
# collide. Requires the stack up + grpcurl. Admin passes the gate with no secret.
set -euo pipefail

GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "RegisterUser suite" "grpcurl missing or identity.proto not at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

json_field() { echo "$1" | grep -o "\"$2\": *\"[^\"]*\"" | head -1 | sed 's/.*: *"//;s/"$//' || true; }

call() { # <json-data>
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H 'x-gridtokenx-role: admin' -d "$1" \
    "$GRPC_ADDR" identity.IdentityService/RegisterUser 2>&1 || true
}

STAMP="$(date +%s)$RANDOM"
USER="greg_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Greg-2025!'
REQ="{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}"

echo "→ RegisterUser with a fresh username"
R=$(call "$REQ")
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "fresh register wrongly hit RBAC denial: ${R:0:160}"
elif echo "$R" | grep -qE '"userId"|"user_id"'; then
  ok "fresh username → RegisterUserResponse with user_id"
  UN=$(json_field "$R" username)
  [ "$UN" = "$USER" ] && ok "echoed username=$USER" || bad "username=$UN (expected $USER)"
  EM=$(json_field "$R" email)
  [ "$EM" = "$EMAIL" ] && ok "echoed email=$EMAIL" || bad "email=$EM (expected $EMAIL)"
else
  bad "fresh register → no user_id in response: ${R:0:200}"
fi

echo "→ RegisterUser with the SAME username (duplicate must fail)"
R=$(call "$REQ")
if echo "$R" | grep -qE '"userId"|"user_id"'; then
  bad "duplicate username → second account created (unique constraint not enforced): ${R:0:160}"
else
  ok "duplicate username → rejected (no second account)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
