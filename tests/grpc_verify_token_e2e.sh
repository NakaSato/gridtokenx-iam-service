#!/usr/bin/env bash
# Live E2E: IdentityService/VerifyToken functional correctness.
#
# grpc_rbac_e2e.sh only proves the RBAC header gate; this proves the RPC's
# actual token logic against a REAL JWT minted by the REST login flow:
#
#   valid JWT   → valid:true, echoes the token's user_id + role
#   garbage JWT → valid:false (RPC succeeds, claims simply not valid — matches
#                 the unit test verify_token_invalid_is_ok_but_not_valid)
#
# This is how the gateway/Trading verify a bearer token out-of-band. Admin role
# passes the gate with no gateway secret. Requires the stack up + grpcurl.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "VerifyToken suite" "grpcurl missing or identity.proto not at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

# `: *` tolerates grpcurl's pretty-printed `"k": "v"` as well as REST's `"k":"v"`.
# `|| true` keeps a no-match from tripping `set -o pipefail`.
json_field() { echo "$1" | grep -o "\"$2\": *\"[^\"]*\"" | head -1 | sed 's/.*: *"//;s/"$//' || true; }

# Dev convenience: clear the per-IP register throttle so the suite can register
# a fresh user. The fix that makes /register 5/hour otherwise starves back-to-
# back e2e runs from a single host.
RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Mint a real JWT: register → verify → login ────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="vtok_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Vtok-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "VerifyToken suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "minted a real JWT via REST login"

call() { # <json-data>
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H 'x-gridtokenx-role: admin' -d "$1" \
    "$GRPC_ADDR" identity.IdentityService/VerifyToken 2>&1 || true
}

echo "→ VerifyToken with the valid JWT"
R=$(call "{\"token\":\"$JWT\"}")
if echo "$R" | grep -q '"valid": true'; then
  ok "valid JWT → valid:true"
  ROLE=$(json_field "$R" role)
  [ "$ROLE" = "user" ] && ok "echoed role=user (registration default)" \
    || bad "echoed role=$ROLE (expected user)"
  echo "$R" | grep -q '"userId"' && ok "response carries user_id" \
    || bad "no user_id in response: ${R:0:160}"
else
  bad "valid JWT → not valid: ${R:0:200}"
fi

echo "→ VerifyToken with a garbage token"
R=$(call '{"token":"not.a.real.jwt"}')
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "garbage token wrongly hit RBAC denial (gate misconfigured): ${R:0:160}"
elif echo "$R" | grep -q '"valid": true'; then
  bad "garbage token → valid:true (must be false)"
else
  ok "garbage token → not valid (RPC ok, claims rejected)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
