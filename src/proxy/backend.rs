// src/proxy/backend.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
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
        let id = format!("{}:{}", url.host_str().unwrap_or("unknown"), 
                        url.port_or_known_default().unwrap_or(80));
        
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
}

#[derive(Debug)]
pub struct BackendMetrics {
    pub active_connections: usize,
    pub total_requests: u64,
    pub failed_requests: u64,
}

// src/proxy/pool.rs
use super::backend::Backend;
use crate::config::BackendConfig;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

#[derive(Clone)]
pub struct BackendPool {
    backends: Arc<DashMap<String, Arc<Backend>>>,
    healthy_backends: Arc<RwLock<Vec<Arc<Backend>>>>,
}

impl BackendPool {
    pub fn new(configs: Vec<BackendConfig>) -> Self {
        let backends = Arc::new(DashMap::new());
        let mut healthy = Vec::new();
        
        for config in configs {
            let backend = Arc::new(Backend::new(
                config.url.clone(),
                config.weight,
                config.max_connections,
            ));
            
            backends.insert(backend.id.clone(), backend.clone());
            healthy.push(backend);
        }
        
        Self {
            backends,
            healthy_backends: Arc::new(RwLock::new(healthy)),
        }
    }
    
    pub async fn get_healthy_backends(&self) -> Vec<Arc<Backend>> {
        self.healthy_backends.read().await.clone()
    }
    
    pub fn get_backend(&self, id: &str) -> Option<Arc<Backend>> {
        self.backends.get(id).map(|b| b.clone())
    }
    
    pub fn all_backends(&self) -> Vec<Arc<Backend>> {
        self.backends.iter().map(|entry| entry.value().clone()).collect()
    }
    
    pub async fn update_healthy_backends(&self) {
        let mut healthy = Vec::new();
        
        for backend in self.backends.iter() {
            if backend.is_healthy().await {
                healthy.push(backend.value().clone());
            }
        }
        
        let mut healthy_backends = self.healthy_backends.write().await;
        *healthy_backends = healthy;
        
        tracing::info!(
            "Updated healthy backends: {}/{} available",
            healthy_backends.len(),
            self.backends.len()
        );
    }
    
    pub async fn add_backend(&self, url: Url, weight: u32, max_connections: usize) {
        let backend = Arc::new(Backend::new(url, weight, max_connections));
        let id = backend.id.clone();
        
        self.backends.insert(id, backend.clone());
        
        // Initially mark as unhealthy until health check passes
        backend.update_health(false).await;
        
        tracing::info!("Added new backend: {}", backend.id);
    }
    
    pub async fn remove_backend(&self, id: &str) -> bool {
        if let Some((_, backend)) = self.backends.remove(id) {
            // Remove from healthy list
            let mut healthy = self.healthy_backends.write().await;
            healthy.retain(|b| b.id != id);
            
            tracing::info!("Removed backend: {}", id);
            true
        } else {
            false
        }
    }
}

// src/proxy/mod.rs
mod proxy;
mod backend;
mod pool;

pub use proxy::{Proxy, ProxyError};
pub use backend::{Backend, HealthStatus, BackendMetrics};
pub use pool::BackendPool;