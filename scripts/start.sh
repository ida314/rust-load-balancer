#!/usr/bin/env bash
# start.sh - Start all services
set -e

echo "🔄 Cleaning up old processes..."
pkill -f "test_backend" || true
pkill -f "target/release/rust-load-balancer" || true
pkill -f "target/debug/rust-load-balancer" || true
sleep 1

echo "🚀 Building release binary..."
cargo build --release --example test_backend
cargo build --release

echo "🌐 Starting backend servers..."
# Start with explicit backend names that match config.yaml
RUST_LOG=error ./target/release/examples/test_backend 8001 backend-8001 &
RUST_LOG=error ./target/release/examples/test_backend 8002 backend-8002 &
RUST_LOG=error ./target/release/examples/test_backend 8003 backend-8003 &

echo "⏳ Waiting for backends to start..."
for port in 8001 8002 8003; do
    count=0
    while ! nc -z localhost $port 2>/dev/null; do
        sleep 0.1
        count=$((count + 1))
        if [ $count -gt 50 ]; then
            echo "❌ Backend on port $port failed to start"
            exit 1
        fi
    done
    echo "✅ Backend on port $port is ready"
done

echo "🔧 Starting load balancer..."
RUST_LOG=rust_load_balancer=info,hyper=warn ./target/release/rust-load-balancer &
LB_PID=$!

echo "⏳ Waiting for load balancer..."
count=0
while ! nc -z localhost 8080 2>/dev/null || ! nc -z localhost 9090 2>/dev/null; do
    sleep 0.1
    count=$((count + 1))
    if [ $count -gt 100 ]; then
        echo "❌ Load balancer failed to start"
        kill $LB_PID 2>/dev/null || true
        exit 1
    fi
done

# Wait a bit more for metrics to initialize
sleep 2

# Test that metrics endpoint works
if curl -sf "http://localhost:9090/metrics" | grep -q "lb_active_connections"; then
    echo "✅ Metrics endpoint is working"
else
    echo "⚠️  Metrics endpoint not returning expected data"
fi

echo ""
echo "✅ All services started successfully!"
echo ""
echo "📊 Services:"
echo "   Load Balancer: http://localhost:8080"
echo "   Metrics:       http://localhost:9090/metrics"
echo "   Backend 1:     http://localhost:8001"
echo "   Backend 2:     http://localhost:8002"
echo "   Backend 3:     http://localhost:8003"
echo ""
echo "🧪 Test with: curl http://localhost:8080/echo"
echo "📈 Run benchmarks with: ./benchmarks/lat_conn.sh"