//! examples/test_backend.rs
//! Run: cargo run --example test_backend -- <port> [name]

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use rand::Rng;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;

#[derive(Clone)]
struct BackendState {
    port:         u16,
    name:         String,
    req_counter:  Arc<AtomicU64>,
    healthy_flag: Arc<AtomicBool>,
    base_delay:   u64,
    jitter_ms:    u64,
    fail_pct:     f64,
}

// ——————————————————————————————————————————
// Request handler
async fn handle(
    req: Request<Body>,
    state: BackendState,
) -> Result<Response<Body>, Infallible> {
    let n = state.req_counter.fetch_add(1, Ordering::SeqCst) + 1;
    let path = req.uri().path().to_owned();

    // /health is always fast
    if path == "/health" {
        if state.healthy_flag.load(Ordering::SeqCst) {
            return Ok(Response::new(Body::from("OK")));
        } else {
            return Ok(Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(Body::from("Unhealthy"))
                .unwrap());
        }
    }

    // Simulate latency
    let delay =
        state.base_delay + rand::thread_rng().gen_range(0..=state.jitter_ms);
    if delay > 0 {
        sleep(Duration::from_millis(delay)).await;
    }

    // Simulate failure
    if state.fail_pct > 0.0
        && rand::thread_rng().gen_bool(state.fail_pct / 100.0)
    {
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Injected failure"))
            .unwrap());
    }

    let body = format!(
        r#"{{"backend":"{}","port":{},"req":{},"path":"{}","delay_ms":{}}}"#,
        state.name, state.port, n, path, delay
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("X-Backend-Name", state.name.clone())
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