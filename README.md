# Rust Load Balancer

A HTTP load balancer written in Rust with advanced features for reliability and observability.

## Features

- **Multiple Load Balancing Algorithms**
  - Round Robin
  **Expanding to**
  - Least Connections
  - Weighted Random
  - Random
  - IP Hash (session affinity)

- **Health Checking**
  - Periodic health checks with configurable intervals
  - Automatic backend removal/addition based on health
  - Configurable failure/success thresholds

- **Circuit Breaker Pattern**
  - Per-backend circuit breakers
  - Three states: Closed, Open, Half-Open
  - Automatic recovery testing

- **Retry Strategy**
  - Configurable retry attempts
  - Exponential backoff with jitter
  - Smart retry decisions based on error types

- **Prometheus Metrics**
  - Request count, latency, and size metrics
  - Backend health and connection metrics
  - Circuit breaker state tracking

- **Configuration**
  - YAML/JSON configuration files
  - Hot-reloadable configuration (future enhancement)

## Building and Running


### Prerequisites

- Rust 1.70+ (install from https://rustup.rs/)
- Backend servers to load balance

### Build

```bash
cargo build --release
```

### Run Test Backends

First, start some test backend servers:

```bash
# Terminal 1
cargo run --example test_backend -- 8001

# Terminal 2
cargo run --example test_backend -- 8002

# Terminal 3
cargo run --example test_backend -- 8003
```

### Run Load Balancer

```bash
# Using default config.yaml
cargo run --release

# Or specify a config file
cargo run --release -- path/to/config.yaml
```

The load balancer will start on `http://localhost:8080`

## Configuration

Create a `config.yaml` file (see the example in the artifacts) with your backend servers and preferences.

### Key Configuration Options

- **Load Balancer Algorithm**: Choose from available algorithms
- **Backends**: List of backend servers with weights and connection limits
- **Health Check**: Configure health check intervals and thresholds
- **Circuit Breaker**: Set failure thresholds and timeout durations
- **Retry**: Configure retry attempts and backoff strategies
- **Metrics**: Enable Prometheus metrics endpoint

## Testing

### Basic Functionality Test

```bash
# Send requests to the load balancer
curl http://localhost:8080/
curl http://localhost:8080/api/test
curl http://localhost:8080/health
```

### Load Testing

```bash
# Install Apache Bench (ab) or use wrk/hey
ab -n 10000 -c 100 http://localhost:8080/

# Or with hey
hey -n 10000 -c 100 http://localhost:8080/
```

### Viewing Metrics

```bash
# Prometheus metrics are available at:
curl http://localhost:9090/metrics
```

Key metrics to monitor:
- `lb_requests_total` - Total requests by method, status, and backend
- `lb_request_duration_seconds` - Request latency histogram
- `lb_backend_health_status` - Backend health (1=healthy, 0=unhealthy)
- `lb_circuit_breaker_state` - Circuit breaker states
- `lb_active_connections` - Current active connections

### Testing Failure Scenarios

1. **Backend Failure**: Stop one of the backend servers and observe:
   - Health checks marking it as unhealthy
   - Requests being routed to remaining healthy backends
   - Metrics showing the unhealthy backend

2. **Circuit Breaker**: Cause repeated failures to trigger circuit breaker:
   - The test backend toggles health every 30 seconds
   - Watch logs for circuit breaker state changes
   - Observe automatic recovery attempts

3. **Connection Limits**: Send many concurrent requests:
   - Use high concurrency in load testing
   - Observe connection limit enforcement
   - Check metrics for active connections

## Monitoring

### Grafana Dashboard

You can import Prometheus metrics into Grafana for visualization:

1. Add Prometheus data source pointing to `http://localhost:9090`
2. Create dashboards with panels for:
   - Request rate and latency
   - Backend health status
   - Circuit breaker states
   - Error rates by backend

### Example Prometheus Queries

```promql
# Request rate by backend
rate(lb_requests_total[5m])

# Average request latency
histogram_quantile(0.95, rate(lb_request_duration_seconds_bucket[5m]))

# Backend health status
lb_backend_health_status

# Circuit breaker open backends
lb_circuit_breaker_state == 1
```

## Architecture

The load balancer is built with a modular, component-based architecture:

- **Config Module**: Handles configuration parsing and validation
- **Proxy Module**: Core request routing and forwarding logic
- **Load Balancer Module**: Pluggable load balancing algorithms
- **Health Module**: Background health checking system
- **Circuit Breaker Module**: Per-backend circuit breaker implementation
- **Retry Module**: Configurable retry strategies
- **Metrics Module**: Prometheus metrics collection

## Performance Tuning

1. **Worker Threads**: Adjust in `main.rs`:
   ```rust
   #[tokio::main(flavor = "multi_thread", worker_threads = 8)]
   ```

2. **Connection Pooling**: Configure in proxy client builder

3. **Backend Limits**: Set appropriate `max_connections` per backend

4. **OS Limits**: Increase file descriptor limits:
   ```bash
   ulimit -n 65536
   ```

## Future Enhancements

- [ ] TLS/SSL support for HTTPS
- [ ] WebSocket support
- [ ] Request/Response transformation
- [ ] Rate limiting
- [ ] Authentication/Authorization
- [ ] Dynamic backend discovery
- [ ] Configuration hot-reloading
- [ ] Distributed tracing support
- [ ] Request logging to file/syslog
- [ ] Admin API for runtime management

## License

MIT
