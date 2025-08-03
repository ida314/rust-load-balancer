# Create integration test script
#!/bin/bash
# tests/integration_test.sh

echo "Starting backend servers..."
cargo run --example test_backend -- 8001 &
cargo run --example test_backend -- 8002 &
cargo run --example test_backend -- 8003 &

sleep 2

echo "Starting load balancer..."
cargo run --release -- config.yaml &
LB_PID=$!

sleep 2

echo "Running test scenarios..."

# Test 1: Basic routing
echo "Test 1: Basic routing"
for i in {1..10}; do
    curl -s http://localhost:8080/ | jq .backend
done

# Test 2: Health check failure
echo "Test 2: Killing backend 8001..."
kill $(lsof -ti:8001)
sleep 15  # Wait for health check
curl -s http://localhost:8080/ | jq .backend

# Test 3: Circuit breaker
echo "Test 3: Testing circuit breaker..."
# Send many requests to trigger failures