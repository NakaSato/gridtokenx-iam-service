#!/usr/bin/env bash
# High-volume registration load generator for iam-service monitoring

BASE="${BASE:-http://localhost:4010}"
TOTAL="${TOTAL:-500}"
CONCURRENCY="${CONCURRENCY:-20}"

echo "=== Registration Load Test ==="
echo "Target      : $BASE"
echo "Total       : $TOTAL users"
echo "Concurrency : $CONCURRENCY"
echo ""

TMPDIR_RESULTS=$(mktemp -d)
START=$(date +%s)

run_batch() {
  local start=$1 end=$2
  for ((i=start; i<=end; i++)); do
    local ts=$(date +%s%N)
    local status
    status=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE/api/v1/auth/register" \
      -H "Content-Type: application/json" \
      -d "{\"username\":\"lu_${i}_${ts}\",\"email\":\"lu_${i}_${ts}@load.test\",\"password\":\"Load@Test123\"}")
    echo "$status" >> "$TMPDIR_RESULTS/$i"
  done
}

export -f run_batch
export BASE TMPDIR_RESULTS

# Dispatch batches
pids=()
for ((i=1; i<=TOTAL; i+=CONCURRENCY)); do
  end=$((i + CONCURRENCY - 1))
  [[ $end -gt $TOTAL ]] && end=$TOTAL
  run_batch "$i" "$end" &
  pids+=($!)
  # Progress indicator
  echo -ne "  Dispatched $end/$TOTAL batches...\r"
done

# Wait for all
for pid in "${pids[@]}"; do wait "$pid"; done
echo ""

END=$(date +%s)
ELAPSED=$((END - START))

# Tally results
PASS=0; FAIL=0
for f in "$TMPDIR_RESULTS"/*; do
  s=$(cat "$f")
  if [[ "$s" == "200" || "$s" == "201" ]]; then ((PASS++)) || true
  else ((FAIL++)) || true
  fi
done
rm -rf "$TMPDIR_RESULTS"

echo "=== Results ==="
echo "  ✅ Success : $PASS"
echo "  ❌ Failed  : $FAIL"
echo "  ⏱  Time    : ${ELAPSED}s"
echo "  📈 RPS     : $((TOTAL / (ELAPSED > 0 ? ELAPSED : 1)))"
echo ""
echo "📊 Grafana: http://localhost:6002/d/gridtokenx-iam-service/gridtokenx-iam-service"
