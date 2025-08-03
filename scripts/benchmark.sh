#!/bin/bash
# scripts/benchmark.sh - Comprehensive benchmark suite

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BACKENDS=(8001 8002 8003)
LB_PORT=8080
METRICS_PORT=9090
RESULTS_DIR="benchmark_results/$(date +%Y%m%d_%H%M%S)"
WARMUP_DURATION="10s"
TEST_DURATION="60s"

# Create results directory
mkdir -p "$RESULTS_DIR"

echo -e "${GREEN}=== Rust Load Balancer Benchmark Suite ===${NC}"
echo "Results will be saved to: $RESULTS_DIR"

# Function to start backends
start_backends() {
    echo -e "${YELLOW}Starting backend servers...${NC}"
    for port in "${BACKENDS[@]}"; do
        cargo run --example test_backend -- $port > "$RESULTS_DIR/backend_$port.log" 2>&1 &
        echo $! > "$RESULTS_DIR/backend_$port.pid"
        echo "Started backend on port $port (PID: $(cat "$RESULTS_DIR/backend_$port.pid"))"
    done
    sleep 3
}

# Function to stop backends
stop_backends() {
    echo -e "${YELLOW}Stopping backend servers...${NC}"
    for port in "${BACKENDS[@]}"; do
        if [ -f "$RESULTS_DIR/backend_$port.pid" ]; then
            kill $(cat "$RESULTS_DIR/backend_$port.pid") 2>/dev/null || true
            rm "$RESULTS_DIR/backend_$port.pid"
        fi
    done
}

# Function to start load balancer
start_load_balancer() {
    echo -e "${YELLOW}Starting load balancer...${NC}"
    cargo run --release -- config.yaml > "$RESULTS_DIR/load_balancer.log" 2>&1 &
    echo $! > "$RESULTS_DIR/load_balancer.pid"
    echo "Load balancer started (PID: $(cat "$RESULTS_DIR/load_balancer.pid"))"
    sleep 5
}

# Function to stop load balancer
stop_load_balancer() {
    echo -e "${YELLOW}Stopping load balancer...${NC}"
    if [ -f "$RESULTS_DIR/load_balancer.pid" ]; then
        kill $(cat "$RESULTS_DIR/load_balancer.pid") 2>/dev/null || true
        rm "$RESULTS_DIR/load_balancer.pid"
    fi
}

# Function to collect metrics
collect_metrics() {
    echo -e "${YELLOW}Collecting Prometheus metrics...${NC}"
    curl -s "http://localhost:$METRICS_PORT/metrics" > "$RESULTS_DIR/metrics_$1.txt"
}

# Function to run wrk benchmark
run_wrk_benchmark() {
    local test_name=$1
    local connections=$2
    local threads=$3
    local duration=$4
    
    echo -e "${GREEN}Running benchmark: $test_name${NC}"
    echo "Connections: $connections, Threads: $threads, Duration: $duration"
    
    # Warmup
    echo "Warming up for $WARMUP_DURATION..."
    wrk -t$threads -c$connections -d$WARMUP_DURATION "http://localhost:$LB_PORT/" > /dev/null 2>&1
    
    # Actual test
    echo "Running test..."
    wrk -t$threads -c$connections -d$duration \
        --latency \
        --script scripts/wrk_report.lua \
        "http://localhost:$LB_PORT/" > "$RESULTS_DIR/${test_name}.txt"
    
    # Collect metrics after test
    collect_metrics "$test_name"
    
    # Parse and display results
    echo -e "${GREEN}Results for $test_name:${NC}"
    grep -E "Requests/sec:|Latency" "$RESULTS_DIR/${test_name}.txt"
    echo ""
}

# Function to run different algorithm tests
test_algorithms() {
    echo -e "${GREEN}=== Testing Different Algorithms ===${NC}"
    
    for algo in round_robin least_connections weighted random; do
        echo -e "${YELLOW}Testing $algo algorithm${NC}"
        
        # Update config
        sed -i.bak "s/algorithm: .*/algorithm: \"$algo\"/" config.yaml
        
        # Restart load balancer
        stop_load_balancer
        start_load_balancer
        
        # Run benchmark
        run_wrk_benchmark "algorithm_$algo" 200 8 $TEST_DURATION
    done
    
    # Restore original config
    mv config.yaml.bak config.yaml
}

# Function to test scaling
test_scaling() {
    echo -e "${GREEN}=== Testing Connection Scaling ===${NC}"
    
    for connections in 50 100 200 400 800 1600; do
        run_wrk_benchmark "scaling_${connections}c" $connections 8 "30s"
    done
}

# Function to test backend failure
test_backend_failure() {
    echo -e "${GREEN}=== Testing Backend Failure Handling ===${NC}"
    
    # Start with all backends
    run_wrk_benchmark "all_backends_healthy" 200 8 "30s"
    
    # Kill one backend
    echo -e "${YELLOW}Killing backend on port ${BACKENDS[0]}...${NC}"
    kill $(cat "$RESULTS_DIR/backend_${BACKENDS[0]}.pid")
    sleep 15  # Wait for health check to detect
    
    run_wrk_benchmark "one_backend_down" 200 8 "30s"
    
    # Kill another backend
    echo -e "${YELLOW}Killing backend on port ${BACKENDS[1]}...${NC}"
    kill $(cat "$RESULTS_DIR/backend_${BACKENDS[1]}.pid")
    sleep 15
    
    run_wrk_benchmark "two_backends_down" 200 8 "30s"
    
    # Restart backends
    echo -e "${YELLOW}Restarting backends...${NC}"
    cargo run --example test_backend -- ${BACKENDS[0]} > "$RESULTS_DIR/backend_${BACKENDS[0]}.log" 2>&1 &
    echo $! > "$RESULTS_DIR/backend_${BACKENDS[0]}.pid"
    cargo run --example test_backend -- ${BACKENDS[1]} > "$RESULTS_DIR/backend_${BACKENDS[1]}.log" 2>&1 &
    echo $! > "$RESULTS_DIR/backend_${BACKENDS[1]}.pid"
    sleep 20  # Wait for health checks
    
    run_wrk_benchmark "backends_recovered" 200 8 "30s"
}

# Function to run stress test
run_stress_test() {
    echo -e "${GREEN}=== Running Stress Test ===${NC}"
    
    # Use vegeta for constant rate testing
    if command -v vegeta &> /dev/null; then
        echo "Running Vegeta stress test..."
        echo "GET http://localhost:$LB_PORT/" | \
            vegeta attack -duration=5m -rate=1000 | \
            vegeta report > "$RESULTS_DIR/vegeta_stress.txt"
        
        echo "GET http://localhost:$LB_PORT/" | \
            vegeta attack -duration=5m -rate=1000 | \
            vegeta plot > "$RESULTS_DIR/latency_plot.html"
    else
        echo "Vegeta not installed, skipping constant rate test"
    fi
}

# Function to generate report
generate_report() {
    echo -e "${GREEN}=== Generating Report ===${NC}"
    
    cat > "$RESULTS_DIR/report.md" << EOF
# Load Balancer Benchmark Report

Date: $(date)

## Configuration
- Backend Servers: ${#BACKENDS[@]}
- Test Duration: $TEST_DURATION
- Load Balancer Port: $LB_PORT

## Results Summary

### Algorithm Comparison
EOF

    for algo in round_robin least_connections weighted random; do
        if [ -f "$RESULTS_DIR/algorithm_$algo.txt" ]; then
            echo "#### $algo" >> "$RESULTS_DIR/report.md"
            echo '```' >> "$RESULTS_DIR/report.md"
            grep -E "Requests/sec:|Latency" "$RESULTS_DIR/algorithm_$algo.txt" >> "$RESULTS_DIR/report.md"
            echo '```' >> "$RESULTS_DIR/report.md"
        fi
    done
    
    echo -e "${GREEN}Report generated: $RESULTS_DIR/report.md${NC}"
}

# Cleanup function
cleanup() {
    echo -e "${RED}Cleaning up...${NC}"
    stop_load_balancer
    stop_backends
    exit
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

# Main execution
main() {
    # Check dependencies
    if ! command -v wrk &> /dev/null; then
        echo -e "${RED}Error: wrk is not installed. Please install it first.${NC}"
        exit 1
    fi
    
    # Start services
    start_backends
    start_load_balancer
    
    # Run benchmarks
    test_algorithms
    test_scaling
    test_backend_failure
    run_stress_test
    
    # Generate report
    generate_report
    
    echo -e "${GREEN}=== Benchmark Complete ===${NC}"
    echo "Results saved to: $RESULTS_DIR"
}

# Run main function
main