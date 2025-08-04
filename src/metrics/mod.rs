// src/metrics/mod.rs
mod collector;

pub use collector::{Timer, MetricsCollector, MetricsRegistry};
