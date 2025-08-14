// src/metrics/collector.rs
use prometheus::{
    Encoder, IntCounterVec, IntGauge, IntGaugeVec, HistogramVec, HistogramOpts,
    Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;

pub struct MetricsRegistry {
    registry: Registry,
    collector: Arc<MetricsCollector>,
}

impl MetricsRegistry {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();
        let collector = Arc::new(MetricsCollector::new(&registry)?);
        
        Ok(Self {
            registry,
            collector,
        })
    }
    
    pub fn collector(&self) -> Arc<MetricsCollector> {
        self.collector.clone()
    }
    
    pub fn gather(&self) -> Vec<u8> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        buffer
    }
}

pub struct MetricsCollector {
    // Request metrics
    pub requests_total: IntCounterVec,
    pub request_duration_seconds: HistogramVec,
    pub request_size_bytes: HistogramVec,
    pub response_size_bytes: HistogramVec,
    
    // Backend metrics
    pub backend_requests_total: IntCounterVec,
    pub backend_request_duration_seconds: HistogramVec,
    pub backend_connections_active: IntGaugeVec,
    pub backend_health_status: IntGaugeVec,
    
    // Circuit breaker metrics
    pub circuit_breaker_state: IntGaugeVec,
    pub circuit_breaker_failures_total: IntCounterVec,
    
    // System metrics
    pub active_connections: IntGauge,
    pub healthy_backends: IntGauge,
    pub total_backends: IntGauge,
}

impl MetricsCollector {
    pub fn new(registry: &Registry) -> Result<Self> {
        // Request metrics
        let requests_total = IntCounterVec::new(
            Opts::new("lb_requests_total", "Total number of requests"),
            &["method", "status_code", "backend"],
        )?;
        registry.register(Box::new(requests_total.clone()))?;
        
        let request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "lb_request_duration_seconds",
                "Request duration in seconds",
            ),
            &["method", "status_code", "backend"],
        )?;
        registry.register(Box::new(request_duration_seconds.clone()))?;
        
        let request_size_bytes = HistogramVec::new(
            HistogramOpts::new("lb_request_size_bytes", "Request size in bytes"),
            &["method"],
        )?;
        registry.register(Box::new(request_size_bytes.clone()))?;
        
        let response_size_bytes = HistogramVec::new(
            HistogramOpts::new(
                "lb_response_size_bytes",
                "Response size in bytes",
            ),
            &["method", "status_code"],
        )?;
        registry.register(Box::new(response_size_bytes.clone()))?;
        
        // Backend metrics
        let backend_requests_total = IntCounterVec::new(
            Opts::new("lb_backend_requests_total", "Total backend requests"),
            &["backend", "status"],
        )?;
        registry.register(Box::new(backend_requests_total.clone()))?;
        
        let backend_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "lb_backend_request_duration_seconds",
                "Backend request duration",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(backend_request_duration_seconds.clone()))?;
        
        let backend_connections_active = IntGaugeVec::new(
            Opts::new(
                "lb_backend_connections_active",
                "Active backend connections",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(backend_connections_active.clone()))?;
        
        let backend_health_status = IntGaugeVec::new(
            Opts::new(
                "lb_backend_health_status",
                "Backend health status (1=healthy, 0=unhealthy)",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(backend_health_status.clone()))?;
        
        // Circuit breaker metrics
        let circuit_breaker_state = IntGaugeVec::new(
            Opts::new(
                "lb_circuit_breaker_state",
                "Circuit breaker state (0=closed, 1=open, 2=half-open)",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(circuit_breaker_state.clone()))?;
        
        let circuit_breaker_failures_total = IntCounterVec::new(
            Opts::new(
                "lb_circuit_breaker_failures_total",
                "Total circuit breaker failures",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(circuit_breaker_failures_total.clone()))?;
        
        // System metrics
        let active_connections =
            IntGauge::new("lb_active_connections", "Total active connections")?;
        registry.register(Box::new(active_connections.clone()))?;
        
        let healthy_backends =
            IntGauge::new("lb_healthy_backends", "Number of healthy backends")?;
        registry.register(Box::new(healthy_backends.clone()))?;
        
        let total_backends =
            IntGauge::new("lb_total_backends", "Total number of backends")?;
        registry.register(Box::new(total_backends.clone()))?;
        
        Ok(Self {
            requests_total,
            request_duration_seconds,
            request_size_bytes,
            response_size_bytes,
            backend_requests_total,
            backend_request_duration_seconds,
            backend_connections_active,
            backend_health_status,
            circuit_breaker_state,
            circuit_breaker_failures_total,
            active_connections,
            healthy_backends,
            total_backends,
        })
    }
    
    pub fn record_request(
        &self,
        method: &str,
        status_code: u16,
        backend: &str,
        duration: std::time::Duration,
    ) {
        let status = status_code.to_string();
        self.requests_total
            .with_label_values(&[method, &status, backend])
            .inc();
        
        self.request_duration_seconds
            .with_label_values(&[method, &status, backend])
            .observe(duration.as_secs_f64());
    }
    
    pub fn record_backend_request(
        &self,
        backend: &str,
        success: bool,
        duration: std::time::Duration,
    ) {
        let status = if success { "success" } else { "failure" };
        self.backend_requests_total
            .with_label_values(&[backend, status])
            .inc();
        
        self.backend_request_duration_seconds
            .with_label_values(&[backend])
            .observe(duration.as_secs_f64());
    }
    
    pub fn update_backend_connections(&self, backend: &str, count: i64) {
        self.backend_connections_active
            .with_label_values(&[backend])
            .set(count);
    }
    
    pub fn update_backend_health(&self, backend: &str, healthy: bool) {
        let value = if healthy { 1 } else { 0 };
        self.backend_health_status
            .with_label_values(&[backend])
            .set(value);
    }
    
    pub fn update_circuit_breaker_state(
        &self,
        backend: &str,
        state: crate::circuit_breaker::CircuitBreakerState,
    ) {
        let value = match state {
            crate::circuit_breaker::CircuitBreakerState::Closed => 0,
            crate::circuit_breaker::CircuitBreakerState::Open => 1,
            crate::circuit_breaker::CircuitBreakerState::HalfOpen => 2,
        };
        
        self.circuit_breaker_state
            .with_label_values(&[backend])
            .set(value);
    }
    
    pub fn increment_active_connections(&self) {
        self.active_connections.inc();
    }
    
    pub fn decrement_active_connections(&self) {
        self.active_connections.dec();
    }
    
    pub fn update_backend_counts(&self, healthy: usize, total: usize) {
        self.healthy_backends.set(healthy as i64);
        self.total_backends.set(total as i64);
    }
}

// Helper for timing operations
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
    
    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}
