#!/usr/bin/env bash
# Live E2E: /health/ready dependency-degraded behavior.
#
# /health/ready checks Postgres + Redis (vs /health/live = process only). This
# proves the readiness probe actually flips when a dependency drops:
#
#   default      : assert ready=200 and /health/live=200 (non-destructive)
#   DESTRUCTIVE=1 : stop Redis → expect /health/ready 503 while /health/live
#                   stays 200 (liveness independent of deps) → restart Redis →
#                   ready recovers to 200. Redis is always restarted on exit.
#
# DESTRUCTIVE=1 briefly stops the Redis container — only run against dev/local.
set -euo pipefail

BASE="${IAM_BASE:-http://localhost:4010}"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"

PASS=0; FAIL=0; SKIP=0
ok()   { echo "✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "❌ $1"; FAIL=$((FAIL+1)); }
skip() { echo "⏭️  SKIP $1 — $2"; SKIP=$((SKIP+1)); }
code() { curl -s -o /dev/null -w '%{http_code}' "$BASE$1"; }

# ── Baseline: healthy stack ───────────────────────────────────────────────────
RC=$(code /health/ready); LC=$(code /health/live)
[ "$RC" = "200" ] && ok "/health/ready → 200 (deps up)" || bad "/health/ready → $RC (expected 200)"
[ "$LC" = "200" ] && ok "/health/live  → 200" || bad "/health/live → $LC (expected 200)"

if [ "${DESTRUCTIVE:-0}" != "1" ]; then
  skip "degraded-dependency case" "set DESTRUCTIVE=1 to stop Redis and assert /health/ready flips to 503"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
  [ "$FAIL" -eq 0 ]; exit $?
fi

if ! docker inspect "$REDIS_CTR" >/dev/null 2>&1; then
  skip "degraded-dependency case" "Redis container '$REDIS_CTR' not found"
  echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
  [ "$FAIL" -eq 0 ]; exit $?
fi

# Always bring Redis back, even on error/interrupt.
restore_redis() { echo "→ restoring Redis"; docker start "$REDIS_CTR" >/dev/null 2>&1 || true; }
trap restore_redis EXIT

echo "→ stopping Redis to force a degraded dependency"
docker stop "$REDIS_CTR" >/dev/null
sleep 2

RC=$(code /health/ready); LC=$(code /health/live)
if [ "$RC" = "503" ] || [ "$RC" = "500" ]; then
  ok "/health/ready → $RC with Redis down (not ready)"
else
  bad "/health/ready → $RC with Redis down (expected 503)"
fi
[ "$LC" = "200" ] && ok "/health/live → 200 with Redis down (liveness independent of deps)" \
  || bad "/health/live → $LC with Redis down (expected 200 — liveness must not depend on Redis)"

echo "→ restarting Redis and waiting for readiness to recover"
docker start "$REDIS_CTR" >/dev/null
trap - EXIT
RECOVERED=0
for _ in $(seq 15); do
  if [ "$(code /health/ready)" = "200" ]; then RECOVERED=1; break; fi
  sleep 1
done
[ "$RECOVERED" = "1" ] && ok "/health/ready recovered to 200 after Redis restart" \
  || bad "/health/ready did not recover within 15s"

echo "── $PASS passed, $FAIL failed, $SKIP skipped ──"
[ "$FAIL" -eq 0 ]
