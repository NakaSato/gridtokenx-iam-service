#!/usr/bin/env bash
# Live E2E: /metrics Prometheus scrape surface.
#
# get_metrics() (startup.rs:308) renders the metrics-exporter-prometheus handle.
# The HTTP middleware (middleware/metrics.rs) feeds `iam_*` series on every
# request. This pins that the endpoint is scrapeable and actually carries the
# instrumented counters:
#
#   GET /metrics                 → 200, Prometheus text (# TYPE … lines)
#   after an instrumented call   → iam_http_requests_total present & countable
#
# NOTE: /metrics is public on the service port (:4010); APISIX restricts it to
# internal CIDRs at the gateway (not exercised here). Requires iam-service.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }

echo "→ generate one instrumented request, then scrape"
curl -s -o /dev/null "$BASE/health" || true

R=$(curl -s -w '\n%{http_code}' "$BASE/metrics")
BODY=$(echo "$R" | sed '$d'); CODE=$(echo "$R" | tail -1)

[ "$CODE" = "200" ] && ok "GET /metrics → 200" || bad "GET /metrics → $CODE (expected 200)"

# Use a here-string, NOT `echo "$BODY" | grep -q …`: grep -q exits on the first
# match without draining stdin, so echo takes a SIGPIPE (141); with `set -o
# pipefail` that makes the whole pipeline "fail" and trips the `|| bad` branch
# even though the pattern matched. The big 700+-line body makes it deterministic.
grep -qa '# TYPE' <<<"$BODY" \
  && ok "body is Prometheus exposition format (# TYPE present)" \
  || bad "no '# TYPE' lines — not Prometheus format: ${BODY:0:120}"

grep -qa 'iam_http_requests_total' <<<"$BODY" \
  && ok "iam_http_requests_total series exported" \
  || bad "iam_http_requests_total missing — middleware not feeding the recorder"

# The render must be a bounded text payload, not an error string.
LINES=$(echo "$BODY" | wc -l | tr -d ' ')
[ "$LINES" -gt 1 ] && ok "scrape carries $LINES lines of series data" \
  || bad "scrape body suspiciously short ($LINES lines)"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
