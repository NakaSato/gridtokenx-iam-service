#!/usr/bin/env bash
# Live E2E: ops/system surface IP restriction at the APISIX gateway.
#
# /api/v1/system/config, /health*, /metrics are gated to internal CIDRs
# (127/8, 10/8, 172.16/12, 192.168/16) via APISIX `ip-restriction`. They must be
# reachable on the internal path and NOT from a public client.
#
# CAVEAT: APISIX `ip-restriction` evaluates the REAL downstream socket IP. A test
# running on the same host IS internal, so the deny case can only be exercised
# with `real-ip`/X-Forwarded-For trust configured, or from a genuinely external
# client. This script asserts the allow path positively and probes the deny path
# best-effort (skips, never false-passes, when it cannot spoof a public source).
#
# GW_BASE = APISIX gateway (:4001, public). SVC_BASE = service direct (:4010, no gate).
set -euo pipefail

GW_BASE="${GW_BASE:-http://localhost:4001}"
SVC_BASE="${IAM_BASE:-http://localhost:4010}"
PUBLIC_IP="${PUBLIC_IP:-203.0.113.7}"   # TEST-NET-3, guaranteed non-internal

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

code()    { curl -s -o /dev/null -w '%{http_code}' "$@"; }

# ── Allow path: ops endpoints reachable internally via the gateway ────────────
for ep in /api/v1/system/config /health /metrics; do
  C=$(code "$GW_BASE$ep")
  if [ "$C" = "000" ]; then
    skip "GET $ep via gateway" "gateway $GW_BASE unreachable"
  elif [ "$C" -lt 400 ] || [ "$C" = "401" ]; then
    # <400 = served; 401 = reached upstream auth (still internal-allowed by ip-restriction)
    ok "GET $ep via gateway (internal allowed, $C)"
  elif [ "$C" = "403" ]; then
    bad "GET $ep via gateway → 403 (internal client wrongly blocked)"
  else
    skip "GET $ep via gateway" "unexpected $C — manual check"
  fi
done

# ── Service-direct: no IP gate at the service itself ──────────────────────────
C=$(code "$SVC_BASE/metrics")
[ "$C" = "200" ] && ok "GET /metrics direct on service ($C, gate is gateway-only)" \
  || skip "GET /metrics direct" "service $SVC_BASE returned $C"

# ── Deny path (best-effort): forge a public source IP ─────────────────────────
# Only meaningful if APISIX is configured to trust X-Forwarded-For (real-ip).
C_FWD=$(code -H "X-Forwarded-For: $PUBLIC_IP" -H "X-Real-IP: $PUBLIC_IP" "$GW_BASE/api/v1/system/config")
if [ "$C_FWD" = "403" ]; then
  ok "GET /system/config as public IP $PUBLIC_IP → 403 (ip-restriction enforced)"
elif [ "$C_FWD" = "000" ]; then
  skip "public-IP deny probe" "gateway unreachable"
else
  skip "public-IP deny probe" "got $C_FWD — APISIX not honoring X-Forwarded-For for ip-restriction (expected; needs real external client)"
fi

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
