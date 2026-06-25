#!/usr/bin/env bash
# Live E2E: liveness/health probes asserted as test subjects.
#
# /health/live is hit ~9× across the suite but only ever as a readiness *poll*,
# never asserted. health_ready_degraded_e2e.sh owns /health/ready. This pins the
# two unconditional probes (startup.rs:256/302):
#
#   GET /health       → 200 {"status":"ok","service":"gridtokenx-iam"}
#   GET /health/live  → 200 {"status":"alive"}   (process up; no dep checks)
#
# Both are dependency-free, so they must answer 200 whenever the process is up —
# even when Postgres/Redis are down (that is what /health/ready is for).
# Requires iam-service on :4010.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"

PASS=0; FAIL=0; SKIP=0
ok()  { echo "✅ $1"; PASS=$((PASS+1)); }
bad() { echo "❌ $1"; FAIL=$((FAIL+1)); }

probe() { curl -s -w '\n%{http_code}' "$BASE$1"; }

echo "→ GET /health"
R=$(probe /health); BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "200" ] && ok "/health → 200" || bad "/health → $CODE (expected 200)"
echo "$BODY" | grep -q '"status":"ok"' && ok "/health body status=ok" || bad "/health body: ${BODY:0:120}"
echo "$BODY" | grep -q '"service":"gridtokenx-iam"' && ok "/health names the service" \
  || bad "/health missing service field: ${BODY:0:120}"

echo "→ GET /health/live"
R=$(probe /health/live); BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)
[ "$CODE" = "200" ] && ok "/health/live → 200" || bad "/health/live → $CODE (expected 200)"
echo "$BODY" | grep -q '"status":"alive"' && ok "/health/live body status=alive" \
  || bad "/health/live body: ${BODY:0:120}"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
