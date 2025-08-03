//
// src/proxy/mod.rs
//
mod proxy;
mod backend;
mod pool;

pub use proxy::{Proxy, ProxyError};
pub use backend::{Backend, HealthStatus, BackendMetrics};
pub use pool::BackendPool;
