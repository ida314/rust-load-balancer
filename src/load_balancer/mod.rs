// src/load_balancer/mod.rs
mod round_robin;
mod algorithm;

pub use algorithm::LoadBalancer; // trait
pub use round_robin::RoundRobinBalancer;
pub use crate::config::LoadBalancerAlgorithm; // enum exposed if needed

use crate::config::LoadBalancerAlgorithm as ConfigAlgorithm;
use std::sync::Arc;

pub fn create_load_balancer(algorithm: ConfigAlgorithm) -> Arc<dyn LoadBalancer> {
    match algorithm {
        ConfigAlgorithm::RoundRobin => Arc::new(RoundRobinBalancer::new()),
        other => {
            tracing::warn!(
                "Unsupported load balancing algorithm {:?}, falling back to round robin",
                other
            );
            Arc::new(RoundRobinBalancer::new())
        }
    }
}
 