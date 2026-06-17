#!/usr/bin/env bash
# Live E2E: GET /me/wallets/{id} + DELETE /me/wallets/{id} (unlink) contract.
#
# wallet_primary_e2e.sh covers PATCH; these two verbs were otherwise untested.
# unlink_wallet (auth_service.rs:642) deletes only NON-primary wallets:
#
#   link secondary (is_primary:false)  → 200, returns its id
#   GET    {secondary_id}              → 200, echoes that wallet
#   DELETE {secondary_id}              → 200 unlinked
#   GET    {secondary_id} again        → 404 (gone — delete really happened)
#   DELETE {primary_id}                → 400 "Cannot delete primary wallet"
#   GET/DELETE {random uuid}           → 404 (not owned by caller)
#
# Secondary link/get/delete need no chain; the primary-guard checks need the
# auto-provisioned custodial wallet and degrade to skip if Chain Bridge is down.
# /me routes require a service role → admin. Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'
RANDID="00000000-0000-4000-8000-000000000000"
# link_wallet validates the address as a real 32-byte Solana pubkey AND it is
# globally unique — mint a fresh valid base58 pubkey per run so reruns (and the
# sibling gRPC suite) don't collide. Needs python3 for the base58 encode.
gen_pubkey() {
  python3 - <<'PY'
import os
b=os.urandom(32)
a='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
n=int.from_bytes(b,'big'); s=''
while n>0:
    n,r=divmod(n,58); s=a[r]+s
pad=0
for c in b:
    if c==0: pad+=1
    else: break
print('1'*pad+s)
PY
}

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
code_get()  { curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me/wallets/$1" -H "$AUTH" -H "$ROLE_HDR"; }
code_del()  { curl -s -o /dev/null -w '%{http_code}' -X DELETE "$BASE/api/v1/me/wallets/$1" -H "$AUTH" -H "$ROLE_HDR"; }

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

# ── Fresh verified account → JWT ──────────────────────────────────────────────
STAMP="$(date +%s)$RANDOM"
USER="wunl_${STAMP}"; EMAIL="${USER}@example.com"; PW='GridTokenX-$Wunl-2025!'
curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"email\":\"$EMAIL\",\"password\":\"$PW\"}" >/dev/null
curl -s "$BASE/api/v1/auth/verify?token=verify_$EMAIL" >/dev/null
LOGIN=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
  -d "{\"username\":\"$USER\",\"password\":\"$PW\"}")
JWT=$(json_field "$LOGIN" access_token)
if [ -z "$JWT" ]; then
  skip "wallet unlink suite" "could not mint a JWT via login: ${LOGIN:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
AUTH="authorization: Bearer $JWT"

# ── Secondary lifecycle (no chain needed) ─────────────────────────────────────
if ! command -v python3 >/dev/null 2>&1; then
  skip "wallet unlink suite" "python3 needed to mint a valid base58 pubkey for the link"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
LINK_ADDR="$(gen_pubkey)"

echo "→ link a secondary wallet (is_primary:false)"
LK=$(curl -s -X POST "$BASE/api/v1/me/wallets" -H "$AUTH" -H "$ROLE_HDR" \
  -H 'content-type: application/json' \
  -d "{\"wallet_address\":\"$LINK_ADDR\",\"label\":\"e2e-unlink\",\"is_primary\":false}")
SECID=$(json_field "$LK" id)
if [ -z "$SECID" ]; then
  bad "link secondary → no wallet id: ${LK:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; [ "$FAIL" -eq 0 ]; exit $?
fi
ok "secondary linked (id=$SECID)"

echo "→ GET the secondary by id"
R=$(curl -s -w '\n%{http_code}' "$BASE/api/v1/me/wallets/$SECID" -H "$AUTH" -H "$ROLE_HDR")
BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "200" ] && ok "GET {secondary_id} → 200" || bad "GET {secondary_id} → $CODE (expected 200)"
[ "$(json_field "$BODY" id)" = "$SECID" ] && ok "echoed the requested wallet id" \
  || bad "GET returned a different wallet: ${BODY:0:160}"

echo "→ DELETE (unlink) the secondary"
C=$(code_del "$SECID")
[ "$C" = "200" ] && ok "DELETE {secondary_id} → 200" || bad "DELETE {secondary_id} → $C (expected 200)"

echo "→ GET the secondary again — must be gone"
C=$(code_get "$SECID")
[ "$C" = "404" ] && ok "GET deleted wallet → 404 (delete confirmed)" \
  || bad "deleted wallet still resolvable → $C (expected 404)"

echo "→ unowned / nonexistent id"
C=$(code_get "$RANDID")
[ "$C" = "404" ] && ok "GET unowned id → 404" || bad "GET unowned → $C (expected 404)"
C=$(code_del "$RANDID")
[ "$C" = "404" ] && ok "DELETE unowned id → 404" || bad "DELETE unowned → $C (expected 404)"

# ── Primary-delete guard (needs the custodial primary) ────────────────────────
echo "→ primary-wallet delete guard"
WL=$(curl -s "$BASE/api/v1/me/wallets" -H "$AUTH" -H "$ROLE_HDR")
PRIMID=$(json_field "$WL" id)
if [ -z "$PRIMID" ] || ! echo "$WL" | grep -q '"is_primary":true'; then
  skip "primary-delete guard" "no custodial primary wallet (Chain Bridge down?)"
else
  C=$(code_del "$PRIMID")
  [ "$C" = "400" ] && ok "DELETE primary → 400 (cannot delete primary)" \
    || bad "DELETE primary → $C (expected 400)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
