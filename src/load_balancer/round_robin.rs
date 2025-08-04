// src/load_balancer/round_robin.rs
use crate::load_balancer::LoadBalancer;
use crate::proxy::Backend;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct RoundRobinBalancer {
    counter: AtomicUsize,
}

impl RoundRobinBalancer {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl LoadBalancer for RoundRobinBalancer {
    async fn select_backend(
        &self,
        backends: &[Arc<Backend>],
        _client_addr: Option<SocketAddr>,
    ) -> Option<Arc<Backend>> {
        if backends.is_empty() {
            return None;
        }
        
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % backends.len();
        Some(backends[index].clone())
    }
    
    fn name(&self) -> &'static str {
        "round_robin"
    }
}