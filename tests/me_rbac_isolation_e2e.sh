#!/usr/bin/env bash
# Live E2E: /me RBAC role-gate + cross-user wallet isolation.
#
# jwt_tamper_e2e.sh pins the *bearer* negative space (forged/absent token) but
# always sends an admin role header. This pins the two orthogonal guards:
#
#   RBAC:  valid bearer, NO x-gridtokenx-role  → 401/403  (get_me require_any)
#          valid bearer + role                 → 200       (baseline)
#   Isolation: user A cannot see/mutate user B's wallet by id —
#          A GET    {B_wallet_id}              → 404  (scoped to claims.sub)
#          A DELETE {B_wallet_id}              → 404
#          B GET    {B_wallet_id}              → 200  (owner sanity check)
#
# B's wallet is a *secondary* link (is_primary:false) → no chain needed; the
# address is a freshly minted valid base58 pubkey (python3). /me routes need a
# service role → admin. Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
ROLE_HDR='x-gridtokenx-role: admin'

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4 || true; }
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

# mint a fresh verified account, echo its bearer token
mint_jwt() { # <prefix> → access_token (or empty)
  local u="$1_$(date +%s)$RANDOM" e pw='GridTokenX-$Iso-2025!'
  e="${u}@example.com"
  curl -s -X POST "$BASE/api/v1/auth/register" -H 'content-type: application/json' \
    -d "{\"username\":\"$u\",\"email\":\"$e\",\"password\":\"$pw\"}" >/dev/null
  curl -s "$BASE/api/v1/auth/verify?token=verify_$e" >/dev/null
  local login
  login=$(curl -s -X POST "$BASE/api/v1/auth/login" -H 'content-type: application/json' \
    -d "{\"username\":\"$u\",\"password\":\"$pw\"}")
  json_field "$login" access_token
}

RL_CTR="${REDIS_CTR:-gridtokenx-redis}"
if docker exec "$RL_CTR" redis-cli ping >/dev/null 2>&1; then
  docker exec "$RL_CTR" redis-cli --scan --pattern 'iam:rate_limit:*register*' \
    | xargs -r -n50 docker exec -i "$RL_CTR" redis-cli DEL >/dev/null 2>&1 || true
fi

JWT_A="$(mint_jwt isoA)"
if [ -z "$JWT_A" ]; then
  skip "rbac/isolation suite" "could not mint a JWT for user A"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; exit 0
fi
AUTH_A="authorization: Bearer $JWT_A"

# ── RBAC role gate ────────────────────────────────────────────────────────────
echo "→ /me with valid bearer but NO role header"
C=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me" -H "$AUTH_A")
case "$C" in
  401|403) ok "missing role header → $C (RBAC fail-closed)";;
  *)       bad "missing role → $C (expected 401/403)";;
esac

echo "→ /me with valid bearer + role (baseline)"
C=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me" -H "$AUTH_A" -H "$ROLE_HDR")
[ "$C" = "200" ] && ok "bearer + role → 200" || bad "bearer + role → $C (expected 200)"

# ── Cross-user wallet isolation ───────────────────────────────────────────────
if ! command -v python3 >/dev/null 2>&1; then
  skip "wallet isolation" "python3 needed to mint a valid base58 pubkey for the link"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; [ "$FAIL" -eq 0 ]; exit $?
fi

JWT_B="$(mint_jwt isoB)"
if [ -z "$JWT_B" ]; then
  skip "wallet isolation" "could not mint a JWT for user B"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; [ "$FAIL" -eq 0 ]; exit $?
fi
AUTH_B="authorization: Bearer $JWT_B"

echo "→ user B links a secondary wallet"
LK=$(curl -s -X POST "$BASE/api/v1/me/wallets" -H "$AUTH_B" -H "$ROLE_HDR" \
  -H 'content-type: application/json' \
  -d "{\"wallet_address\":\"$(gen_pubkey)\",\"label\":\"e2e-iso-B\",\"is_primary\":false}")
BID=$(json_field "$LK" id)
if [ -z "$BID" ]; then
  skip "wallet isolation" "B could not link a secondary wallet: ${LK:0:160}"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"; [ "$FAIL" -eq 0 ]; exit $?
fi
ok "user B owns wallet id=$BID"

echo "→ user A GET B's wallet by id"
C=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me/wallets/$BID" -H "$AUTH_A" -H "$ROLE_HDR")
[ "$C" = "404" ] && ok "A GET B-wallet → 404 (not leaked)" || bad "A GET B-wallet → $C (expected 404)"

echo "→ user A DELETE B's wallet by id"
C=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE "$BASE/api/v1/me/wallets/$BID" -H "$AUTH_A" -H "$ROLE_HDR")
[ "$C" = "404" ] && ok "A DELETE B-wallet → 404 (cannot mutate)" || bad "A DELETE B-wallet → $C (expected 404)"

echo "→ owner sanity: B GET own wallet"
C=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/v1/me/wallets/$BID" -H "$AUTH_B" -H "$ROLE_HDR")
[ "$C" = "200" ] && ok "B GET own wallet → 200" || bad "B GET own wallet → $C (expected 200)"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
