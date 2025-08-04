// src/health/mod.rs
mod checker;
mod status;

pub use checker::{HealthChecker, HealthCheckResult};
pub use status::HealthStatus;
