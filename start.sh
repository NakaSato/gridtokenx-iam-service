#!/usr/bin/env bash
set -e

BINARY="./target/debug/gridtokenx-iam-service"
LOG="/tmp/iam-service.log"

# Kill existing
pkill -9 -f "gridtokenx-iam-service" 2>/dev/null || true
sleep 1

# Build if binary missing
if [[ ! -f "$BINARY" ]]; then
  echo "Building..."
  cargo build -p gridtokenx-iam-service
fi

# Start
OTEL_ENABLED=false \
CHAIN_BRIDGE_INSECURE=true \
CHAIN_BRIDGE_URL=http://localhost:5040 \
DATABASE_URL="postgresql://gridtokenx_user:gridtokenx_password@localhost:7001/gridtokenx" \
REDIS_URL="redis://localhost:7010" \
IAM_PORT=4010 \
JWT_SECRET="dev-jwt-secret-key-minimum-32-characters-long-for-development-2025" \
JWT_EXPIRATION=86400 \
ENCRYPTION_SECRET="dev-encryption-secret-key-32-chars-long-12345" \
API_KEY_SECRET="dev-api-key-secret-key-32-chars-long-67890" \
ENVIRONMENT=development \
TEST_MODE=true \
GATEWAY_SECRET="gridtokenx-gateway-secret-2025" \
SOLANA_RPC_URL="http://localhost:8899" \
"$BINARY" > "$LOG" 2>&1 &

PID=$!
echo "PID: $PID"

for i in $(seq 1 15); do
  sleep 2
  if curl -s http://localhost:4010/health 2>/dev/null | grep -q "ok"; then
    echo "✅ IAM service up (${i}x2s)"
    curl -s http://localhost:4010/health
    exit 0
  fi
  tail -1 "$LOG"
done

echo "❌ Failed to start. Log:"
tail -20 "$LOG"
exit 1
