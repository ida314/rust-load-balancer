// src/load_balancer/algorithm.rs
use crate::proxy::Backend;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;

#[async_trait]
pub trait LoadBalancer: Send + Sync {
    async fn select_backend(
        &self,
        backends: &[Arc<Backend>],
        client_addr: Option<SocketAddr>,
    ) -> Option<Arc<Backend>>;
    
    fn name(&self) -> &'static str;
}

pub use crate::config::LoadBalancerAlgorithm;