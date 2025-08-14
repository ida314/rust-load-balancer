use crate::{
    circuit_breaker::CircuitBreakerManager,
    config::Config,
    health::HealthChecker,
    load_balancer,
    metrics::{MetricsCollector, Timer},
    proxy::{Backend, BackendPool},
    retry::{RetryStrategy, RetryDecision},
};
use anyhow::Result;
use hyper::{
    client::HttpConnector, Body, Client, Request, Response, StatusCode, Uri,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct Proxy {
    config: Config,
    pool: Arc<BackendPool>,
    load_balancer: Arc<dyn load_balancer::LoadBalancer>,
    health_checker: Arc<HealthChecker>,
    circuit_breakers: Arc<CircuitBreakerManager>,
    retry_strategy: RetryStrategy,
    client: Client<HttpConnector>,
    metrics: Arc<MetricsCollector>,
}

impl Proxy {
    pub fn new(
        config: Config,
        pool: Arc<BackendPool>,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        // Create HTTP client with proper settings
        let mut http = HttpConnector::new();
        http.set_connect_timeout(Some(Duration::from_secs(5)));
        http.set_keepalive(Some(Duration::from_secs(60)));

        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(50)
            .build::<_, Body>(http);
        
        let load_balancer = load_balancer::create_load_balancer(config.load_balancer.algorithm);
        
        // Pass metrics to HealthChecker
        let health_checker = Arc::new(HealthChecker::new(
            config.health_check.clone(),
            pool.clone(),
            Some(metrics.clone()),
        ));
        
        let circuit_breakers = Arc::new(CircuitBreakerManager::new(
            config.circuit_breaker.clone(),
        ));
        
        let retry_strategy = RetryStrategy::new(config.retry.clone());
        
        // Update metrics with initial backend count
        let backends = pool.all_backends();
        metrics.update_backend_counts(0, backends.len());
        
        Self {
            config,
            pool,
            load_balancer,
            health_checker,
            circuit_breakers,
            retry_strategy,
            client,
            metrics,
        }
    }
    
    pub fn start_health_checker(&self) {
        let health_checker = self.health_checker.clone();
        tokio::spawn(async move {
            health_checker.start().await;
        });
    }
    
    pub async fn handle(&self, req: Request<Body>) -> Result<Response<Body>, ProxyError> {
        let request_id = Uuid::new_v4();
        let timer = Timer::new();
        
        // Record request size
        let method = req.method().clone();
        if let Some(content_length) = req.headers().get("content-length") {
            if let Ok(size) = content_length.to_str().unwrap_or("0").parse::<f64>() {
                self.metrics.request_size_bytes
                    .with_label_values(&[method.as_str()])
                    .observe(size);
            }
        }
        
        // Extract client address for IP hash algorithm
        let client_addr = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse().ok());
        
        let uri_path = req.uri().path().to_string();
        
        info!(
            request_id = %request_id,
            method = %method,
            path = %uri_path,
            "Handling request"
        );
        
        self.metrics.increment_active_connections();
        
        let result = self.handle_with_retry(req, client_addr, &request_id).await;
        
        self.metrics.decrement_active_connections();
        
        // Record metrics including response size
        match &result {
            Ok(response) => {
                let status = response.status().as_u16();
                let backend_id = response
                    .headers()
                    .get("x-backend-id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown");
                
                // Record response size
                if let Some(content_length) = response.headers().get("content-length") {
                    if let Ok(size) = content_length.to_str().unwrap_or("0").parse::<f64>() {
                        self.metrics.response_size_bytes
                            .with_label_values(&[method.as_str(), &status.to_string()])
                            .observe(size);
                    }
                }
                
                self.metrics.record_request(
                    method.as_str(),
                    status,
                    backend_id,
                    timer.elapsed(),
                );
                
                info!(
                    request_id = %request_id,
                    status = status,
                    backend = backend_id,
                    duration_ms = timer.elapsed().as_millis(),
                    "Request completed"
                );
            }
            Err(_e) => {
                self.metrics.record_request(
                    method.as_str(),
                    503,
                    "none",
                    timer.elapsed(),
                );
                
                error!(
                    request_id = %request_id,
                    error = %_e,
                    duration_ms = timer.elapsed().as_millis(),
                    "Request failed"
                );
            }
        }
        
        result
    }
    
    async fn handle_with_retry(
        &self,
        req: Request<Body>,
        client_addr: Option<std::net::SocketAddr>,
        request_id: &Uuid,
    ) -> Result<Response<Body>, ProxyError> {
        let (parts, body) = req.into_parts();
        let body_bytes = hyper::body::to_bytes(body).await
            .map_err(|e| ProxyError::RequestError(e.to_string()))?;
        
        self.retry_strategy
            .execute_with_decision(
                || async {
                    // Rebuild request for each retry
                    let mut req_builder = Request::builder()
                        .method(parts.method.clone())
                        .uri(parts.uri.clone());
                    
                    for (key, value) in &parts.headers {
                        req_builder = req_builder.header(key, value);
                    }
                    
                    let req = req_builder
                        .body(Body::from(body_bytes.clone()))
                        .map_err(|e| ProxyError::RequestError(e.to_string()))?;
                    
                    self.proxy_request(req, client_addr, request_id).await
                },
                |error| {
                    match error {
                        ProxyError::NoHealthyBackends => RetryDecision::Retry,
                        ProxyError::BackendError(_) => RetryDecision::Retry,
                        ProxyError::Timeout => RetryDecision::Retry,
                        _ => RetryDecision::NoRetry,
                    }
                },
            )
            .await
    }
    
    async fn proxy_request(
        &self,
        req: Request<Body>,
        client_addr: Option<std::net::SocketAddr>,
        request_id: &Uuid,
    ) -> Result<Response<Body>, ProxyError> {
        // Get healthy backends
        let healthy_backends = self.pool.get_healthy_backends().await;
        
        if healthy_backends.is_empty() {
            warn!("No healthy backends available");
            return Err(ProxyError::NoHealthyBackends);
        }
        
        // Select backend using load balancer
        let backend = self
            .load_balancer
            .select_backend(&healthy_backends, client_addr)
            .await
            .ok_or(ProxyError::NoHealthyBackends)?;
        
        debug!(
            request_id = %request_id,
            backend = %backend.id,
            "Selected backend"
        );
        
        // Check circuit breaker
        let circuit_breaker = self.circuit_breakers.get_or_create(&backend.id);
        
        if !circuit_breaker.call_permitted().await {
            warn!(
                request_id = %request_id,
                backend = %backend.id,
                "Circuit breaker is open"
            );
            return Err(ProxyError::CircuitBreakerOpen(backend.id.clone()));
        }
        
        // Check connection limit
        if !backend.increment_connections() {
            warn!(
                request_id = %request_id,
                backend = %backend.id,
                "Backend connection limit reached"
            );
            return Err(ProxyError::ConnectionLimitReached(backend.id.clone()));
        }
        
        // Update metrics
        self.metrics.update_backend_connections(
            &backend.id,
            backend.active_connections() as i64,
        );
        
        // Forward request
        let result = self.forward_request(req, &backend, request_id).await;
        
        // Decrement connections
        backend.decrement_connections();
        self.metrics.update_backend_connections(
            &backend.id,
            backend.active_connections() as i64,
        );
        
        // Record circuit breaker result
        match &result {
            Ok(_) => {
                circuit_breaker.record_success().await;
                backend.record_request(true);
            }
            Err(_) => {
                circuit_breaker.record_failure().await;
                backend.record_request(false);
            }
        }
        
        // Update circuit breaker metrics
        self.metrics.update_circuit_breaker_state(
            &backend.id,
            circuit_breaker.get_state().await,
        );
        
        result
    }
    
    async fn forward_request(
        &self,
        mut req: Request<Body>,
        backend: &Backend,
        request_id: &Uuid,
    ) -> Result<Response<Body>, ProxyError> {
        let timer = Timer::new();
        
        // Get the path and query from the original request
        let path_and_query = req.uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        
        // Parse backend URL and replace only the path and query
        let backend_uri = backend.url.as_str()
            .parse::<Uri>()
            .map_err(|e| ProxyError::InvalidUri(format!("Invalid backend URL: {}", e)))?;
        
        // Build new URI with backend's scheme/authority but request's path/query
        let new_uri = Uri::builder()
            .scheme(backend_uri.scheme().unwrap().clone())
            .authority(backend_uri.authority().unwrap().clone())
            .path_and_query(path_and_query)
            .build()
            .map_err(|e| ProxyError::InvalidUri(format!("Failed to build URI: {}", e)))?;
        
        *req.uri_mut() = new_uri;
        
        // Add proxy headers
        let real_ip = req
            .headers()
            .get("x-real-ip")
            .cloned()
            .unwrap_or_else(|| "unknown".parse().unwrap());
        req.headers_mut().insert("x-forwarded-for", real_ip);

        req.headers_mut().insert(
            "x-request-id",
            request_id.to_string().parse().unwrap(),
        );
        
        // Forward request
        debug!(
            request_id = %request_id,
            backend = %backend.id,
            uri = %req.uri(),
            "Forwarding request"
        );
        
        match self.client.request(req).await {
            Ok(mut response) => {
                // Add backend identifier to response
                response.headers_mut().insert(
                    "x-backend-id",
                    backend.id.parse().unwrap(),
                );
                
                self.metrics.record_backend_request(
                    &backend.id,
                    response.status().is_success(),
                    timer.elapsed(),
                );
                
                Ok(response)
            }
            Err(e) => {
                error!(
                    request_id = %request_id,
                    backend = %backend.id,
                    error = %e,
                    "Backend request failed"
                );
                
                self.metrics.record_backend_request(&backend.id, false, timer.elapsed());
                
                Err(ProxyError::BackendError(e.to_string()))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("No healthy backends available")]
    NoHealthyBackends,
    
    #[error("Backend error: {0}")]
    BackendError(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Circuit breaker open for backend: {0}")]
    CircuitBreakerOpen(String),
    
    #[error("Connection limit reached for backend: {0}")]
    ConnectionLimitReached(String),
    
    #[error("Invalid URI: {0}")]
    InvalidUri(String),
    
    #[error("Request error: {0}")]
    RequestError(String),
}

impl From<ProxyError> for Response<Body> {
    fn from(err: ProxyError) -> Self {
        let (status, message) = match &err {
            ProxyError::NoHealthyBackends => (StatusCode::SERVICE_UNAVAILABLE, "No healthy backends available"),
            ProxyError::BackendError(_) => (StatusCode::BAD_GATEWAY, "Backend error"),
            ProxyError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Gateway timeout"),
            ProxyError::CircuitBreakerOpen(_) => (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable"),
            ProxyError::ConnectionLimitReached(_) => (StatusCode::SERVICE_UNAVAILABLE, "Backend overloaded"),
            ProxyError::InvalidUri(_) => (StatusCode::BAD_REQUEST, "Invalid request URI"),
            ProxyError::RequestError(_) => (StatusCode::BAD_REQUEST, "Invalid request"),
        };
        
        Response::builder()
            .status(status)
            .header("x-error", err.to_string())
            .body(Body::from(message))
            .unwrap()
    }
}