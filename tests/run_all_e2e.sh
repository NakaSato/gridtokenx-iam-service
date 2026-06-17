#!/usr/bin/env bash
# Runner: execute every tests/*_e2e.sh in sequence and tally the suite.
#
# The auth surface is rate-limited (login 10/60s, register 5/3600s per IP — see
# rate_limit.rs). Run from one host, ~24 scripts back-to-back blow that budget
# and later scripts 429 mid-flight (JWT-mint skips, false reds). The individual
# scripts only clear their *register* throttle, not *login*. This runner clears
# ALL iam:rate_limit:* keys BEFORE each script so every one starts with a fresh
# budget and the whole suite can go green in a single shot.
#
# Usage:  tests/run_all_e2e.sh                 # all scripts
#         DESTRUCTIVE=1 tests/run_all_e2e.sh   # also run Redis-stop degraded case
#         tests/run_all_e2e.sh grpc_ jwt_      # only scripts matching a prefix/substr
#
# Env: IAM_BASE, GRPC_ADDR, REDIS_CTR pass straight through to each script.
set -uo pipefail   # NOT -e: a failing script must not abort the whole run.

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REDIS_CTR="${REDIS_CTR:-gridtokenx-redis}"

# Optional filters: only run scripts whose basename matches one of the args.
FILTERS=("$@")
matches() {
  [ "${#FILTERS[@]}" -eq 0 ] && return 0
  local b; b="$(basename "$1")"
  for f in "${FILTERS[@]}"; do [[ "$b" == *"$f"* ]] && return 0; done
  return 1
}

clear_throttles() {
  docker exec "$REDIS_CTR" redis-cli --scan --pattern 'iam:rate_limit:*' 2>/dev/null \
    | xargs -r -n50 docker exec -i "$REDIS_CTR" redis-cli DEL >/dev/null 2>&1 || true
}

if ! docker exec "$REDIS_CTR" redis-cli ping >/dev/null 2>&1; then
  echo "⚠️  Redis '$REDIS_CTR' unreachable — cannot clear throttles between scripts;"
  echo "    late scripts may 429. Proceeding anyway."
fi

GREEN=0; RED=0; reds=()
START=$(date +%s)

for f in "$HERE"/*_e2e.sh; do
  [ "$(basename "$f")" = "run_all_e2e.sh" ] && continue   # don't run the runner
  matches "$f" || continue
  name="$(basename "$f" .sh)"
  clear_throttles
  out="$("$f" 2>&1)"; rc=$?
  summary="$(echo "$out" | grep -E '──.*passed|Results:.*passed' | tail -1)"
  [ -z "$summary" ] && summary="(no summary; rc=$rc)"
  if [ "$rc" -eq 0 ]; then
    printf '  ✅ %-34s %s\n' "$name" "$summary"; GREEN=$((GREEN+1))
  else
    printf '  ❌ %-34s %s\n' "$name" "$summary"; RED=$((RED+1)); reds+=("$name")
    # Echo the failing lines so a red is actionable without a rerun.
    echo "$out" | grep -E '❌' | sed 's/^/        /'
  fi
done

ELAPSED=$(( $(date +%s) - START ))
echo "──────────────────────────────────────────────"
echo "Suite: $GREEN green, $RED red  (${ELAPSED}s)"
if [ "$RED" -gt 0 ]; then
  echo "RED: ${reds[*]}"
  exit 1
fi
echo "All e2e scripts passed."
