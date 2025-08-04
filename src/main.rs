// src/main.rs
use anyhow::Result;
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

mod config;
mod server;
mod proxy;
mod load_balancer;
mod health;
mod circuit_breaker;
mod retry;
mod metrics;

use crate::{
    config::Config,
    metrics::{MetricsCollector, MetricsRegistry},
    proxy::{BackendPool, Proxy},
    server::{handler::RequestHandler, ServerBuilder},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rust_load_balancer=debug".parse()?)
                .add_directive("hyper=info".parse()?),
        )
        .init();
    
    // Load configuration
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());
    
    info!("Loading configuration from: {}", config_path);
    let config = config::load_config(&config_path).await?;
    
    // Initialize metrics
    let metrics_registry = MetricsRegistry::new()?;
    let metrics = metrics_registry.collector();
    
    // Create backend pool
    let pool = Arc::new(BackendPool::new(config.backends.clone()));
    
    // Create proxy
    let proxy = Arc::new(Proxy::new(config.clone(), pool, metrics.clone()));
    
    // Start health checker
    proxy.start_health_checker();
    
    // Start metrics server if enabled
    if config.metrics.enabled {
        let metrics_addr: SocketAddr = ([0, 0, 0, 0], config.metrics.port).into();
        start_metrics_server(metrics_addr, metrics_registry, config.metrics.path).await?;
    }
    
    // Create request handler
    let handler = RequestHandler::new(proxy);
    
    // Start main server
    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    info!("Starting load balancer on {}", addr);
    
    ServerBuilder::new(addr)
        .with_handler(handler)
        .serve()
        .await?;
    
    Ok(())
}

async fn start_metrics_server(
    addr: SocketAddr,
    registry: MetricsRegistry,
    path: String,
) -> Result<()> {
    let registry = Arc::new(registry);
    let metrics_path = Arc::new(path); // keep this for logging
    let service_path = metrics_path.clone(); // clone for the service closure

    let make_service = hyper::service::make_service_fn(move |_| {
        let registry = registry.clone();
        let path = service_path.clone();

        async move {
            Ok::<_, Infallible>(hyper::service::service_fn(move |req: Request<Body>| {
                let registry = registry.clone();
                let path = path.clone();

                async move {
                    if req.uri().path() == path.as_str() {
                        let metrics = registry.gather();
                        Ok::<_, Infallible>(
                            Response::builder()
                                .status(StatusCode::OK)
                                .header("Content-Type", "text/plain; version=0.0.4")
                                .body(Body::from(metrics))
                                .unwrap(),
                        )
                    } else {
                        Ok::<_, Infallible>(
                            Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::from("Not Found"))
                                .unwrap(),
                        )
                    }
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    info!(
        "Metrics server listening on http://{}{}",
        addr,
        metrics_path.as_str()
    );

    tokio::spawn(async move {
        if let Err(e) = server.await {
            error!("Metrics server error: {}", e);
        }
    });

    Ok(())
}


// Graceful shutdown handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };
    
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };
    
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    
    info!("Shutdown signal received");
}