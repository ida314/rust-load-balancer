// src/circuit_breaker/breaker.rs

use crate::config::CircuitBreakerConfig;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitBreakerState {
    Closed,     // Normal operation
    Open,       // Failing, rejecting requests
    HalfOpen,   // Testing if service recovered
}

pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitBreakerState>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: RwLock<Option<Instant>>,
    total_requests: AtomicU64,
    failed_requests: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitBreakerState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: RwLock::new(None),
            total_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
        }
    }
    
    pub async fn call_permitted(&self) -> bool {
        let state = self.state.read().await;
        
        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if timeout has passed
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.config.timeout() {
                        drop(state);
                        self.transition_to_half_open().await;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }
    
    pub async fn record_success(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        let state = self.state.read().await;
        
        match *state {
            CircuitBreakerState::Closed => {
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitBreakerState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                
                if success_count >= self.config.success_threshold {
                    drop(state);
                    self.transition_to_closed().await;
                }
            }
            CircuitBreakerState::Open => {
                // Shouldn't happen, but reset if it does
                drop(state);
                self.transition_to_closed().await;
            }
        }
    }
    
    pub async fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
        
        let state = self.state.read().await;
        
        match *state {
            CircuitBreakerState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                
                if failure_count >= self.config.failure_threshold {
                    drop(state);
                    self.transition_to_open().await;
                }
            }
            CircuitBreakerState::HalfOpen => {
                drop(state);
                self.transition_to_open().await;
            }
            CircuitBreakerState::Open => {
                // Already open, update last failure time
                let mut last_failure = self.last_failure_time.write().await;
                *last_failure = Some(Instant::now());
            }
        }
    }
    
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Open;
        
        let mut last_failure = self.last_failure_time.write().await;
        *last_failure = Some(Instant::now());
        
        self.success_count.store(0, Ordering::Relaxed);
        
        tracing::warn!("Circuit breaker opened after {} failures", 
                      self.failure_count.load(Ordering::Relaxed));
    }
    
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::HalfOpen;
        
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        
        tracing::info!("Circuit breaker transitioned to half-open");
    }
    
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Closed;
        
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        
        let mut last_failure = self.last_failure_time.write().await;
        *last_failure = None;
        
        tracing::info!("Circuit breaker closed after successful recovery");
    }
    
    pub async fn get_state(&self) -> CircuitBreakerState {
        *self.state.read().await
    }
    
    pub fn get_metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
pub struct CircuitBreakerMetrics {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub failure_count: u32,
    pub success_count: u32,
}

// Per-backend circuit breaker management
use dashmap::DashMap;

pub struct CircuitBreakerManager {
    breakers: DashMap<String, Arc<CircuitBreaker>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: DashMap::new(),
            config,
        }
    }
    
    pub fn get_or_create(&self, backend_id: &str) -> Arc<CircuitBreaker> {
        self.breakers
            .entry(backend_id.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.config.clone())))
            .clone()
    }
    
    pub fn remove(&self, backend_id: &str) {
        self.breakers.remove(backend_id);
    }
}