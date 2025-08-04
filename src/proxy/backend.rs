// src/proxy/backend.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tokio::sync::RwLock;
use url::Url;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Debug)]
pub struct Backend {
    pub id: String,
    pub url: Url,
    pub weight: u32,
    pub max_connections: usize,
    
    // Runtime state
    active_connections: AtomicUsize,
    total_requests: AtomicU64,
    failed_requests: AtomicU64,
    health_status: RwLock<HealthStatus>,
    last_health_check: RwLock<Option<DateTime<Utc>>>,
    consecutive_failures: AtomicUsize,
    consecutive_successes: AtomicUsize,
}

impl Backend {
    pub fn new(url: Url, weight: u32, max_connections: usize) -> Self {
        let id = format!(
            "{}:{}",
            url.host_str().unwrap_or("unknown"),
            url.port_or_known_default().unwrap_or(80)
        );
        
        Self {
            id,
            url,
            weight,
            max_connections,
            active_connections: AtomicUsize::new(0),
            total_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            health_status: RwLock::new(HealthStatus::Unknown),
            last_health_check: RwLock::new(None),
            consecutive_failures: AtomicUsize::new(0),
            consecutive_successes: AtomicUsize::new(0),
        }
    }
    
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }
    
    pub fn increment_connections(&self) -> bool {
        loop {
            let current = self.active_connections.load(Ordering::Relaxed);
            if current >= self.max_connections {
                return false;
            }
            
            if self.active_connections.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ).is_ok() {
                return true;
            }
        }
    }
    
    pub fn decrement_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }
    
    pub fn record_request(&self, success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if !success {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub async fn is_healthy(&self) -> bool {
        *self.health_status.read().await == HealthStatus::Healthy
    }
    
    pub async fn update_health(&self, healthy: bool) {
        let mut status = self.health_status.write().await;
        *status = if healthy {
            self.consecutive_failures.store(0, Ordering::Relaxed);
            self.consecutive_successes.fetch_add(1, Ordering::Relaxed);
            HealthStatus::Healthy
        } else {
            self.consecutive_successes.store(0, Ordering::Relaxed);
            self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
            HealthStatus::Unhealthy
        };
        
        let mut last_check = self.last_health_check.write().await;
        *last_check = Some(Utc::now());
    }
    
    pub fn get_metrics(&self) -> BackendMetrics {
        BackendMetrics {
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
        }
    }

    // --- safe accessors for health streaks ---
    /// Snapshot of consecutive successful health checks.
    pub fn consecutive_successes(&self) -> usize {
        self.consecutive_successes.load(Ordering::Relaxed)
    }

    /// Snapshot of consecutive failed health checks.
    pub fn consecutive_failures(&self) -> usize {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Convenience: is this backend considered stably healthy given a threshold?
    pub fn is_stably_healthy(&self, threshold: usize) -> bool {
        self.consecutive_successes.load(Ordering::Relaxed) >= threshold
    }

    /// Convenience: is this backend considered stably unhealthy given a threshold?
    pub fn is_stably_unhealthy(&self, threshold: usize) -> bool {
        self.consecutive_failures.load(Ordering::Relaxed) >= threshold
    }
}

#[derive(Debug)]
pub struct BackendMetrics {
    pub active_connections: usize,
    pub total_requests: u64,
    pub failed_requests: u64,
}
