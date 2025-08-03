// src/retry/strategy.rs

use crate::config::RetryConfig;
use anyhow::Result;
use hyper::{Body, Response, StatusCode};
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct RetryStrategy {
    config: RetryConfig,
}

#[derive(Debug)]
pub enum RetryDecision {
    Retry,
    NoRetry,
}

#[derive(Debug, thiserror::Error)]
pub enum RetryError {
    #[error("Maximum retry attempts ({0}) exceeded")]
    MaxAttemptsExceeded(u32),
    
    #[error("Non-retryable error: {0}")]
    NonRetryable(String),
}

impl RetryStrategy {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }
    
    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T, E>(
        &self,
        mut f: F,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;
        
        loop {
            attempt += 1;
            
            match f().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempt >= self.config.max_attempts {
                        warn!(
                            "Retry failed after {} attempts: {}",
                            attempt, error
                        );
                        return Err(error);
                    }
                    
                    let backoff = self.calculate_backoff(attempt);
                    debug!(
                        "Attempt {} failed: {}. Retrying in {:?}",
                        attempt, error, backoff
                    );
                    
                    sleep(backoff).await;
                }
            }
        }
    }
    
    /// Execute with custom retry decision logic
    pub async fn execute_with_decision<F, Fut, T, E>(
        &self,
        mut f: F,
        should_retry: impl Fn(&E) -> RetryDecision,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;
        
        loop {
            attempt += 1;
            
            match f().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    match should_retry(&error) {
                        RetryDecision::NoRetry => {
                            debug!("Error is non-retryable: {}", error);
                            return Err(error);
                        }
                        RetryDecision::Retry => {
                            if attempt >= self.config.max_attempts {
                                warn!(
                                    "Retry failed after {} attempts: {}",
                                    attempt, error
                                );
                                return Err(error);
                            }
                            
                            let backoff = self.calculate_backoff(attempt);
                            debug!(
                                "Attempt {} failed: {}. Retrying in {:?}",
                                attempt, error, backoff
                            );
                            
                            sleep(backoff).await;
                        }
                    }
                }
            }
        }
    }
    
    /// Calculate exponential backoff with jitter
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base = self.config.backoff_base().as_millis() as u64;
        let max = self.config.backoff_max().as_millis() as u64;
        
        // Exponential backoff: base * 2^(attempt - 1)
        let exponential = base.saturating_mul(2u64.saturating_pow(attempt - 1));
        
        // Cap at maximum
        let capped = exponential.min(max);
        
        // Add jitter (0-25% of the calculated backoff)
        let jitter = (capped as f64 * rand::random::<f64>() * 0.25) as u64;
        
        Duration::from_millis(capped + jitter)
    }
    
    /// Determine if an HTTP status code is retryable
    pub fn is_retryable_status(status: StatusCode) -> RetryDecision {
        match status {
            // Retry on server errors and specific client errors
            StatusCode::REQUEST_TIMEOUT |
            StatusCode::TOO_MANY_REQUESTS |
            StatusCode::INTERNAL_SERVER_ERROR |
            StatusCode::BAD_GATEWAY |
            StatusCode::SERVICE_UNAVAILABLE |
            StatusCode::GATEWAY_TIMEOUT => RetryDecision::Retry,
            
            // Don't retry on client errors (except the ones above)
            s if s.is_client_error() => RetryDecision::NoRetry,
            
            // Retry on other server errors
            s if s.is_server_error() => RetryDecision::Retry,
            
            // Don't retry on success or other statuses
            _ => RetryDecision::NoRetry,
        }
    }
    
    /// Helper for retrying HTTP responses
    pub async fn retry_response<F, Fut>(
        &self,
        f: F,
    ) -> Result<Response<Body>, hyper::Error>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<Response<Body>, hyper::Error>>,
    {
        self.execute_with_decision(f, |error| {
            // Always retry on connection errors
            debug!("Checking if error is retryable: {:?}", error);
            RetryDecision::Retry
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    
    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig {
            max_attempts: 3,
            backoff_base_ms: 10,
            backoff_max_ms: 100,
        };
        
        let strategy = RetryStrategy::new(config);
        let counter = AtomicU32::new(0);
        
        let result = strategy.execute(|| async {
            let count = counter.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err("Temporary failure")
            } else {
                Ok("Success")
            }
        }).await;
        
        assert_eq!(result.unwrap(), "Success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
    
    #[tokio::test]
    async fn test_retry_max_attempts() {
        let config = RetryConfig {
            max_attempts: 2,
            backoff_base_ms: 10,
            backoff_max_ms: 100,
        };
        
        let strategy = RetryStrategy::new(config);
        
        let result: Result<(), &str> = strategy.execute(|| async {
            Err("Always fails")
        }).await;
        
        assert!(result.is_err());
    }
}