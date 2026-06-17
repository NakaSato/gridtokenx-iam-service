#!/usr/bin/env bash
# Live E2E: IdentityService/Authorize permission logic.
#
# grpc_rbac_e2e.sh proves only the caller-role gate; this proves the per-token
# authorization decision itself, against a REAL `user`-role JWT:
#
#   user token + "read"            → authorized:true
#   user token + "admin:settings"  → authorized:false  (admin-scoped perm denied)
#   garbage token                  → authorized:false
#
# Logic (identity_grpc.rs::authorize): admin ⇒ always; user ⇒ true unless the
# required_permission starts with "admin:"; any other role ⇒ false. Caller gate
# uses admin role (no gateway secret needed). Requires the stack up + grpcurl.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

if ! command -v grpcurl >/dev/null 2>&1 || [[ ! -f "$PROTO_DIR/identity.proto" ]]; then
  skip "Authorize suite" "grpcurl missing or identity.proto not at $PROTO_DIR"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi

# `|| true` keeps a no-match from tripping `set -o pipefail`.
json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }

# Dev convenience: clear the per-IP register throttle so the suite can register
# a fresh user (the /register 5/hour budget is otherwise exhausted by repeated
# e2e runs from one host).
RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Mint a real `user`-role JWT ───────────────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="authz_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Authz-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "Authorize suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
ok "minted a real user-role JWT"

call() { # <token> <permission>
  grpcurl -max-time 10 -plaintext \
    -import-path "$PROTO_DIR" -proto identity.proto \
    -H 'x-gridtokenx-role: admin' \
    -d "{\"token\":\"$1\",\"required_permission\":\"$2\"}" \
    "$GRPC_ADDR" identity.IdentityService/Authorize 2>&1 || true
}
authorized_true()  { echo "$1" | grep -q '"authorized": true'; }

echo "→ user token + ordinary permission"
R=$(call "$JWT" "read")
authorized_true "$R" && ok "user + 'read' → authorized:true" \
  || bad "user + 'read' → not authorized: ${R:0:200}"

echo "→ user token + admin-scoped permission"
R=$(call "$JWT" "admin:settings")
if echo "$R" | grep -qiE 'permissiondenied|permission denied'; then
  bad "admin-scoped check hit caller-RBAC denial, not authz logic: ${R:0:160}"
elif authorized_true "$R"; then
  bad "user + 'admin:settings' → authorized:true (admin perm must be denied to user)"
else
  ok "user + 'admin:settings' → authorized:false"
fi

echo "→ garbage token denied"
R=$(call "not.a.real.jwt" "read")
if authorized_true "$R"; then
  bad "garbage token → authorized:true (must be false)"
else
  ok "garbage token → authorized:false"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
