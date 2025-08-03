// src/load_balancer/mod.rs
mod round_robin;

pub use round_robin::RoundRobinBalancer;

use crate::config::LoadBalancerAlgorithm as ConfigAlgorithm;
use std::sync::Arc;

pub fn create_load_balancer(algorithm: ConfigAlgorithm) -> Arc<dyn LoadBalancer> {
    match algorithm {
        ConfigAlgorithm::RoundRobin => Arc::new(RoundRobinBalancer::new()),
    }
}