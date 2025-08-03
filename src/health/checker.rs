// src/health/checker.rs
use crate::config::HealthCheckConfig;
use crate::proxy::{Backend, BackendPool};
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::time::{interval, timeout, Duration};
use tracing::{debug, error, info, warn};

pub struct HealthChecker {
    config: HealthCheckConfig,
    pool: Arc<BackendPool>,
    client: Client,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

#[derive(Debug)]
pub struct HealthCheckResult {
    pub backend_id: String,
    pub healthy: bool,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

impl HealthChecker {
    pub fn new(config: HealthCheckConfig, pool: Arc<BackendPool>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
        
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        
        Self {
            config,
            pool,
            client,
            shutdown_tx,
            shutdown_rx,
        }
    }
    
    pub async fn start(self: Arc<Self>) {
        let mut interval = interval(self.config.interval());
        let mut shutdown_rx = self.shutdown_rx.clone();
        
        info!("Starting health checker with interval: {:?}", self.config.interval());
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.check_all_backends().await;
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Health checker shutting down");
                        break;
                    }
                }
            }
        }
    }
    
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
    
    async fn check_all_backends(&self) {
        let backends = self.pool.all_backends();
        let mut tasks = Vec::new();
        
        for backend in backends {
            let checker = self.clone();
            let task = tokio::spawn(async move {
                checker.check_backend(backend).await
            });
            tasks.push(task);
        }
        
        // Wait for all health checks to complete
        let results: Vec<Result<HealthCheckResult, _>> = 
            futures::future::join_all(tasks).await;
        
        // Process results
        let mut healthy_count = 0;
        let mut unhealthy_count = 0;
        
        for result in results {
            match result {
                Ok(Ok(check_result)) => {
                    if check_result.healthy {
                        healthy_count += 1;
                        debug!("Backend {} is healthy", check_result.backend_id);
                    } else {
                        unhealthy_count += 1;
                        warn!("Backend {} is unhealthy: {:?}", 
                              check_result.backend_id, check_result.error);
                    }
                }
                Ok(Err(e)) => {
                    error!("Health check error: {}", e);
                    unhealthy_count += 1;
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                    unhealthy_count += 1;
                }
            }
        }
        
        // Update the healthy backends list
        self.pool.update_healthy_backends().await;
        
        info!("Health check complete: {} healthy, {} unhealthy", 
              healthy_count, unhealthy_count);
    }
    
    async fn check_backend(&self, backend: Arc<Backend>) -> Result<HealthCheckResult> {
        let start = std::time::Instant::now();
        let url = backend.url.join(&self.config.path)?;
        
        let result = timeout(
            self.config.timeout(),
            self.client.get(url.as_str()).send()
        ).await;
        
        let response_time_ms = start.elapsed().as_millis() as u64;
        
        let (healthy, error) = match result {
            Ok(Ok(response)) => {
                let status = response.status();
                if status.is_success() {
                    (true, None)
                } else {
                    (false, Some(format!("HTTP {}", status)))
                }
            }
            Ok(Err(e)) => (false, Some(e.to_string())),
            Err(_) => (false, Some("Request timeout".to_string())),
        };
        
        // Update backend health status
        backend.update_health(healthy).await;
        
        // Check thresholds
        if healthy {
            let successes = backend.consecutive_successes.load(std::sync::atomic::Ordering::Relaxed);
            if successes >= self.config.healthy_threshold as usize {
                if !backend.is_healthy().await {
                    info!("Backend {} is now healthy after {} consecutive successes", 
                          backend.id, successes);
                }
            }
        } else {
            let failures = backend.consecutive_failures.load(std::sync::atomic::Ordering::Relaxed);
            if failures >= self.config.unhealthy_threshold as usize {
                if backend.is_healthy().await {
                    warn!("Backend {} is now unhealthy after {} consecutive failures", 
                          backend.id, failures);
                }
            }
        }
        
        Ok(HealthCheckResult {
            backend_id: backend.id.clone(),
            healthy,
            response_time_ms,
            error,
        })
    }
}
