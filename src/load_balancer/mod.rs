// src/load_balancer/mod.rs
mod round_robin;
mod traits;

pub use traits::LoadBalancer;
use round_robin::RoundRobinBalancer;

use crate::config::LoadBalancerAlgorithm as ConfigAlgorithm;
use std::sync::Arc;

/// Factory function to create a load balancer based on the algorithm
pub fn create_load_balancer(algorithm: ConfigAlgorithm) -> Arc<dyn LoadBalancer> {
    match algorithm {
        ConfigAlgorithm::RoundRobin => Arc::new(RoundRobinBalancer::new()),
        ConfigAlgorithm::WeightedRoundRobin => {
            // TODO: Implement weighted round robin
            Arc::new(RoundRobinBalancer::new())
        }
        ConfigAlgorithm::LeastConnections => {
            // TODO: Implement least connections
            Arc::new(RoundRobinBalancer::new())
        }
        ConfigAlgorithm::IpHash => {
            // TODO: Implement IP hash
            Arc::new(RoundRobinBalancer::new())
        }
    }
}