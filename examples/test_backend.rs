// examples/test_backend.rs
// Run with: cargo run --example test_backend -- <port>

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone)]
struct BackendState {
    port: u16,
    request_count: Arc<AtomicU64>,
    // Simulate unhealthy state
    healthy: Arc<std::sync::atomic::AtomicBool>,
}

async fn handle_request(
    req: Request<Body>,
    state: BackendState,
) -> Result<Response<Body>, Infallible> {
    let count = state.request_count.fetch_add(1, Ordering::SeqCst) + 1;
    let path = req.uri().path();
    
    println!("[Backend {}] Request #{}: {} {}", 
             state.port, count, req.method(), path);
    
    // Handle health check endpoint
    if path == "/health" {
        let healthy = state.healthy.load(Ordering::SeqCst);
        if healthy {
            return Ok(Response::new(Body::from("OK")));
        } else {
            return Ok(Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(Body::from("Unhealthy"))
                .unwrap());
        }
    }
    
    // Simulate some processing time
    sleep(Duration::from_millis(50)).await;
    
    // Return response with backend identifier
    let body = format!(
        "{{\"backend\": {}, \"request_count\": {}, \"path\": \"{}\"}}",
        state.port, count, path
    );
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("X-Backend-Port", state.port.to_string())
        .body(Body::from(body))
        .unwrap())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8001);
    
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    
    let state = BackendState {
        port,
        request_count: Arc::new(AtomicU64::new(0)),
        healthy: Arc::new(std::sync::atomic::AtomicBool::new(true)),
    };
    
    // Clone for the control task
    let control_state = state.clone();
    
    // Spawn a task to toggle health status (for testing)
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            let current = control_state.healthy.load(Ordering::SeqCst);
            control_state.healthy.store(!current, Ordering::SeqCst);
            println!("[Backend {}] Health status changed to: {}", 
                     control_state.port, !current);
        }
    });
    
    let make_svc = make_service_fn(move |_conn| {
        let state = state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, state.clone())
            }))
        }
    });
    
    let server = Server::bind(&addr).serve(make_svc);
    
    println!("Test backend server listening on http://{}", addr);
    println!("Health endpoint: http://{}/health", addr);
    
    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }
    
    Ok(())
}