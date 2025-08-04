// src/retry/mod.rs
mod strategy;

pub use strategy::{RetryStrategy, RetryDecision, RetryError};
