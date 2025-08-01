// ────────────────────────────────
// src/main.rs
// ────────────────────────────────
use std::net::SocketAddr;
use std::sync::Arc;

mod server;
mod proxy;

use server::{ServerBuilder, handler::RequestHandler};
use proxy::Proxy;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    
    // Create the proxy instance
    let proxy = Arc::new(Proxy::new());
    
    // Create the handler with the proxy
    let handler = RequestHandler::new(proxy);
    
    ServerBuilder::new(addr)
        .with_handler(handler)  // Pass the instance, not the type
        .serve()
        .await
        .expect("server failed");
}