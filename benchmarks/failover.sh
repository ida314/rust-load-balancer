#!/usr/bin/env bash
# benchmarks/failover.sh - Fixed version
source "$(dirname "$0")/common.sh"

backend="backend-8002"
port=8002

echo "Testing failover for $backend..."

# Get initial health status
initial_health=$(scrape 'lb_backend_health_status' "{backend=\"${backend}\"}")
echo "Initial health status: $initial_health"

# Kill the backend
echo "Killing $backend (port $port)..."
pid=$(pid_for_port "$port")
if [[ "$pid" == "ERR" ]] || [[ -z "$pid" ]]; then
    echo "Could not find process for port $port"
    exit 1
fi

kill -9 "$pid" 2>/dev/null
echo "Killed PID $pid"

# Wait for health check to detect failure
start=$(date_ms)
max_wait=30000  # 30 seconds max

while true; do
    current_health=$(scrape 'lb_backend_health_status' "{backend=\"${backend}\"}" || echo "1")
    
    if [[ "$current_health" == "0" ]]; then
        elapsed=$(($(date_ms) - start))
        echo "Backend marked unhealthy in ${elapsed}ms"
        break
    fi
    
    elapsed=$(($(date_ms) - start))
    if [[ $elapsed -gt $max_wait ]]; then
        echo "Timeout waiting for backend to be marked unhealthy"
        exit 1
    fi
    
    sleep 0.5
done

# Test that requests still work (failover to other backends)
echo "Testing failover..."
for i in {1..10}; do
    if curl -sf "http://${LB_HOST}:${LB_PORT}/echo" >/dev/null; then
        echo -n "."
    else
        echo -n "X"
    fi
done
echo ""

# Restart the backend
echo "Restarting $backend..."
cargo run --example test_backend -- $port &
new_pid=$!

# Wait for recovery
start=$(date_ms)
while true; do
    current_health=$(scrape 'lb_backend_health_status' "{backend=\"${backend}\"}" || echo "0")
    
    if [[ "$current_health" == "1" ]]; then
        elapsed=$(($(date_ms) - start))
        echo "Backend recovered in ${elapsed}ms"
        break
    fi
    
    elapsed=$(($(date_ms) - start))
    if [[ $elapsed -gt $max_wait ]]; then
        echo "Timeout waiting for backend recovery"
        exit 1
    fi
    
    sleep 0.5
done

echo "Failover test completed successfully!"