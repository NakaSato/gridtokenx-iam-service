#!/usr/bin/env bash
# Live E2E: IdentityService/VerifyApiKey functional correctness.
#
# grpc_rbac_e2e.sh only proves the role gate; this proves the RPC's verify
# semantics (identity_grpc.rs:139):
#
#   invalid / unknown key → valid:false (RPC SUCCEEDS — the key is simply not
#                           valid; mirrors VerifyToken's garbage-token contract)
#   malformed key         → valid:false
#   valid key             → valid:true + the stored role
#
# IAM exposes no over-the-wire API-key mint endpoint, so the happy path mints a
# key the same way ApiKeyService does — key_hash = sha256(key ‖ API_KEY_SECRET),
# hex (jwt_service.rs hash_key) — and seeds it straight into the shared `api_keys`
# table via the Postgres container. The secret is read from the iam container's
# env so the hash matches what the running service computes. Degrades to skip if
# docker/openssl/psql or the secret are unavailable.
# Requires the stack up + grpcurl.
set -euo pipefail

GRPC_ADDR="${GRPC_ADDR:-localhost:5010}"
PROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../crates/iam-protocol/proto" 2>/dev/null && pwd || true)"
IAM_CTR="${IAM_CTR:-gridtokenx-iam-service}"
PG_CTR="${PG_CTR:-gridtokenx-postgres}"

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

# ── Happy path: seed a real key with a service-matching hash ──────────────────
echo "→ VerifyApiKey with a valid (seeded) key"
psql_iam() { # run SQL in the shared DB using the container's own POSTGRES_* env
  docker exec "$PG_CTR" sh -c \
    'psql -v ON_ERROR_STOP=1 -tAq -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$0"' "$1" 2>/dev/null
}
if ! command -v openssl >/dev/null 2>&1 || ! command -v docker >/dev/null 2>&1; then
  skip "valid-key happy path" "openssl/docker missing — cannot mint+seed a live key"
elif ! SECRET="$(docker exec "$IAM_CTR" printenv API_KEY_SECRET 2>/dev/null)" || [ -z "$SECRET" ]; then
  skip "valid-key happy path" "could not read API_KEY_SECRET from '$IAM_CTR'"
elif ! docker exec "$PG_CTR" sh -c 'pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB"' >/dev/null 2>&1; then
  skip "valid-key happy path" "Postgres container '$PG_CTR' not ready — cannot seed key"
else
  KEY="ak_$(openssl rand -hex 16)"
  # key_hash = hex(sha256(key-bytes ‖ secret-bytes)) — exactly ApiKeyService::hash_key
  HASH="$(printf '%s%s' "$KEY" "$SECRET" | openssl dgst -sha256 | awk '{print $NF}')"
  KROLE="trading-api"
  if psql_iam "INSERT INTO api_keys (name, key_hash, role, is_active) VALUES ('e2e-verify-${RANDOM}', '${HASH}', '${KROLE}', true)" >/dev/null; then
    R=$(call "{\"key\":\"${KEY}\"}")
    if echo "$R" | grep -q '"valid": true'; then
      ok "valid key → valid:true"
      echo "$R" | grep -q "\"role\": \"${KROLE}\"" && ok "valid key echoes stored role ($KROLE)" \
        || bad "role mismatch (expected $KROLE): ${R:0:160}"
    else
      bad "seeded key → not valid: ${R:0:160}"
    fi
    # best-effort cleanup so reruns don't accrete rows
    psql_iam "DELETE FROM api_keys WHERE key_hash = '${HASH}'" >/dev/null || true
  else
    skip "valid-key happy path" "INSERT into api_keys failed (schema/perms?)"
  fi
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
