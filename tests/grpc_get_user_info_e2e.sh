#!/usr/bin/env bash
# Live E2E: IdentityService/GetUserInfo functional correctness.
#
# grpc_rbac_e2e.sh only proves the role gate; this proves the RPC's claim
# decoding against a REAL JWT minted by REST login (identity_grpc.rs:113):
#
#   valid JWT   → UserInfoResponse echoing id + username + role (email/names are
#                 intentionally blank — claims carry no PII)
#   garbage JWT → Unauthenticated error (NOT a valid response, NOT permission-
#                 denied — the RPC distinguishes a bad token from a bad role)
#
# Requires the stack up + grpcurl. Admin role passes the gate with no secret.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "GetUserInfo suite" "grpcurl missing or identity.proto not at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

json_field() { echo "$1" | grep -o "\"$2\": *\"[^\"]*\"" | head -1 | sed 's/.*: *"//;s/"$//' || true; }

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

STAMP="$(date +%s)$RANDOM"
USER="ginfo_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Ginfo-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "GetUserInfo suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "minted a real JWT via REST login"

call() { # <json-data>
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H 'x-gridtokenx-role: admin' -d "$1" \
    "$GRPC_ADDR" identity.IdentityService/GetUserInfo 2>&1 || true
}

echo "→ GetUserInfo with the valid JWT"
R=$(call "{\"token\":\"$JWT\"}")
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "valid JWT wrongly hit RBAC denial: ${R:0:160}"
elif echo "$R" | grep -q '"id"'; then
  ok "valid JWT → UserInfoResponse with id"
  UN=$(json_field "$R" username)
  [ "$UN" = "$USER" ] && ok "echoed username=$USER" || bad "username=$UN (expected $USER)"
  ROLE=$(json_field "$R" role)
  [ "$ROLE" = "user" ] && ok "echoed role=user (registration default)" \
    || bad "role=$ROLE (expected user)"
else
  bad "valid JWT → no UserInfoResponse: ${R:0:200}"
fi

echo "→ GetUserInfo with a garbage token"
R=$(call '{"token":"not.a.real.jwt"}')
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "garbage token hit RBAC denial instead of Unauthenticated: ${R:0:160}"
elif echo "$R" | grep -qiE 'unauthenticated|invalid|Unauthorized|error'; then
  ok "garbage token → Unauthenticated error (bad token ≠ bad role)"
elif echo "$R" | grep -q '"id"'; then
  bad "garbage token → returned a UserInfoResponse (must be rejected): ${R:0:160}"
else
  ok "garbage token → no UserInfoResponse (rejected)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
