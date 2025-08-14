# Rust Load Balancer Benchmark Guide

## Prerequisites

```bash
# Install dependencies
cargo install --locked cargo-watch
brew install wrk  # macOS
# or
sudo apt-get install wrk  # Linux

# Build everything
cargo build --release
cargo build --release --example test_backend
```

## Quick Start

```bash
# 1. Start all services
./start.sh

# 2. Verify services
curl http://localhost:8080/echo
curl http://localhost:9090/metrics

# 3. Run all benchmarks
./run_benchmarks.sh
```

## Individual Benchmarks

### 1. Latency & Connections
Tests P99 latency at different connection levels.

```bash
./benchmarks/lat_conn.sh
```

**Expected Output:**
```
CONN  P99_LAT(ms)   MAX_CONN
 100         3.45        100
 500         5.12        500
1000         8.73       1000
2000        15.21       2000
```

### 2. Throughput
Measures maximum data transfer rate.

```bash
./benchmarks/throughput.sh
```

**Expected Output:**
```
Approx throughput: 8.52 GB/s
Prometheus bytes_total: 268435456000
```

### 3. Failover
Tests backend failure detection and recovery time.

```bash
./benchmarks/failover.sh
```

**Expected Output:**
```
Killing backend-8002...
Backend marked unhealthy in 5234ms
Testing failover: ..........
Backend recovered in 10156ms
```

### 4. Load Distribution
Verifies weighted round-robin distribution.

```bash
./benchmarks/distribution.sh
```

## Troubleshooting

### Benchmarks show "NA" or no data
```bash
# Check metrics endpoint
curl -s http://localhost:9090/metrics | grep -E "(lb_active_connections|lb_backend_health)"

# Check backends are responding
for port in 8001 8002 8003; do
  curl -s http://localhost:$port/echo
done
```

### Common Fixes

1. **No metrics data**: HealthChecker needs metrics instance
2. **Wrong backend names**: Ensure config.yaml uses `backend-8001` format
3. **Throughput shows 0**: Record response sizes in proxy
4. **Distribution off**: Implement weighted_round_robin algorithm

### Clean Restart
```bash
pkill -f "test_backend"
pkill -f "rust-load-balancer"
./start.sh
```

## Performance Targets

| Metric | Target | Excellent |
|--------|--------|-----------|
| P99 Latency @ 1K conn | < 20ms | < 10ms |
| Throughput | > 1 GB/s | > 5 GB/s |
| Failover Detection | < 10s | < 5s |
| Distribution Deviation | < 5% | < 2% |
| Active Connections | 10K+ | 50K+ |

## Advanced Benchmarks

```bash
# Long-running stability test
wrk -t8 -c1000 -d5m --latency http://localhost:8080/echo

# Memory usage under load
./benchmarks/memory_profile.sh

# Connection pool efficiency
./benchmarks/connection_pooling.sh

# Latency percentiles
./benchmarks/latency_percentiles.sh
```