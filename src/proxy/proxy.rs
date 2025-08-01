// ────────────────────────────────
// src/proxy/proxy.rs
// Proxy component that handles HTTP request forwarding
// ────────────────────────────────

use hyper::{Body, Request, Response};

pub struct Proxy {
    // TODO: Add router, backend pool, metrics, etc.
}

impl Proxy {
    pub fn new() -> Self {
        Self {
            // TODO: Initialize with dependencies
        }
    }

    pub async fn handle(&self, _req: Request<Body>) -> Result<Response<Body>, ProxyError> {
        // TODO: Implement actual proxy logic
        // For now, return a simple response
        Ok(Response::builder()
            .status(501)
            .body(Body::from("Not implemented yet"))
            .unwrap())
    }
}

// Custom error type for proxy operations
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("No healthy backends available")]
    NoHealthyBackends,
    
    #[error("Backend error: {0}")]
    BackendError(String),
    
    #[error("Request timeout")]
    Timeout,
    
    // TODO: Add more error variants as needed
}

// Convert ProxyError to Hyper Response for error handling
impl From<ProxyError> for Response<Body> {
    fn from(err: ProxyError) -> Self {
        let (status, message) = match err {
            ProxyError::NoHealthyBackends => (503, "No healthy backends available"),
            ProxyError::BackendError(_) => (502, "Bad gateway"),
            ProxyError::Timeout => (504, "Gateway timeout"),
        };
        
        Response::builder()
            .status(status)
            .body(Body::from(message))
            .unwrap()
    }
}