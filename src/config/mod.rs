// src/config/mod.rs
mod models;

pub use models::*;

use anyhow::{Context, Result};
use std::path::Path;

/// Load configuration from a file (YAML or JSON)
pub async fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let contents = tokio::fs::read_to_string(path)
        .await
        .context("Failed to read config file")?;
    
    let config: Config = if path.extension().and_then(|s| s.to_str()) == Some("yaml") 
        || path.extension().and_then(|s| s.to_str()) == Some("yml") {
        serde_yaml::from_str(&contents).context("Failed to parse YAML config")?
    } else {
        serde_json::from_str(&contents).context("Failed to parse JSON config")?
    };
    
    config.validate()?;
    Ok(config)
}

// src/config/models.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;
use anyhow::{bail, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub load_balancer: LoadBalancerConfig,
    pub backends: Vec<BackendConfig>,
    pub health_check: HealthCheckConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub retry: RetryConfig,
    pub metrics: MetricsConfig,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.backends.is_empty() {
            bail!("At least one backend must be configured");
        }
        
        for (i, backend) in self.backends.iter().enumerate() {
            if backend.weight == 0 {
                bail!("Backend {} has invalid weight: 0", i);
            }
            
            if backend.max_connections == 0 {
                bail!("Backend {} has invalid max_connections: 0", i);
            }
        }
        
        if self.health_check.interval_secs == 0 {
            bail!("Health check interval must be greater than 0");
        }
        
        if self.circuit_breaker.failure_threshold == 0 {
            bail!("Circuit breaker failure threshold must be greater than 0");
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoadBalancerConfig {
    #[serde(default = "default_algorithm")]
    pub algorithm: LoadBalancerAlgorithm,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalancerAlgorithm {
    RoundRobin,
    LeastConnections,
    Weighted,
    Random,
    IpHash,
}

fn default_algorithm() -> LoadBalancerAlgorithm {
    LoadBalancerAlgorithm::RoundRobin
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackendConfig {
    pub url: Url,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

fn default_weight() -> u32 {
    1
}

fn default_max_connections() -> usize {
    100
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthCheckConfig {
    #[serde(default = "default_health_interval")]
    pub interval_secs: u64,
    #[serde(default = "default_health_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: u32,
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: u32,
    #[serde(default = "default_health_path")]
    pub path: String,
}

fn default_health_interval() -> u64 { 10 }
fn default_health_timeout() -> u64 { 3 }
fn default_unhealthy_threshold() -> u32 { 3 }
fn default_healthy_threshold() -> u32 { 2 }
fn default_health_path() -> String { "/health".to_string() }

impl HealthCheckConfig {
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.interval_secs)
    }
    
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitBreakerConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_success_threshold")]
    pub success_threshold: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_failure_threshold() -> u32 { 5 }
fn default_success_threshold() -> u32 { 2 }
fn default_timeout_secs() -> u64 { 60 }

impl CircuitBreakerConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_backoff_base_ms")]
    pub backoff_base_ms: u64,
    #[serde(default = "default_backoff_max_ms")]
    pub backoff_max_ms: u64,
}

fn default_max_attempts() -> u32 { 3 }
fn default_backoff_base_ms() -> u64 { 100 }
fn default_backoff_max_ms() -> u64 { 5000 }

impl RetryConfig {
    pub fn backoff_base(&self) -> Duration {
        Duration::from_millis(self.backoff_base_ms)
    }
    
    pub fn backoff_max(&self) -> Duration {
        Duration::from_millis(self.backoff_max_ms)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,
    #[serde(default = "default_metrics_port")]
    pub port: u16,
    #[serde(default = "default_metrics_path")]
    pub path: String,
}

fn default_metrics_enabled() -> bool { true }
fn default_metrics_port() -> u16 { 9090 }
fn default_metrics_path() -> String { "/metrics".to_string() }