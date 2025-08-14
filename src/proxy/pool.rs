//
// src/proxy/pool.rs
//
use super::backend::Backend;
use crate::config::BackendConfig;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
            let backend = Arc::new(Backend::new(&config));
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
    
    pub async fn add_backend(&self, config: BackendConfig) {
            let backend = Arc::new(Backend::new(&config));
            let id = backend.id.clone();
            
            self.backends.insert(id.clone(), backend.clone());
            
            // Initially mark as unhealthy until health check passes
            backend.update_health(false).await;
            tracing::info!("Added new backend: {}", id);
        }
    
    pub async fn remove_backend(&self, id: &str) -> bool {
        if let Some((_, _backend)) = self.backends.remove(id) {
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