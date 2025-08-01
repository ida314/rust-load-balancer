// ────────────────────────────────
// src/server/builder.rs
// ────────────────────────────────
use crate::server::listener::bind_tcp;
use std::net::SocketAddr;
use anyhow::Result;
use hyper::{server::conn::Http, Body, Request, Response};
use tower::Service;

/// Builder pattern so `main.rs` can inject its Proxy (or any handler).
pub struct ServerBuilder<H>
where
    H: Service<Request<Body>, Response = Response<Body>> + Send + Clone + 'static,
    H::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    H::Future: Send + 'static,
{
    addr: SocketAddr,
    handler: Option<H>,
}

impl<H> ServerBuilder<H>
where
    H: Service<Request<Body>, Response = Response<Body>> + Send + Clone + 'static,
    H::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    H::Future: Send + 'static,
{
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr, handler: None }
    }

    /// Inject your request handler (usually wraps `proxy::Proxy`).
    pub fn with_handler(mut self, handler: H) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Consume the builder, boot the TCP listener, spawn Hyper tasks.
    pub async fn serve(self) -> Result<()> {
        let handler = self.handler.expect("handler must be set via with_handler()");

        // 1️⃣ Bind the TCP socket (plain or TLS can be swapped later).
        let listener = bind_tcp(self.addr).await?;
        tracing::info!("HTTP server listening on {}", self.addr);

        loop {
            let (stream, peer) = listener.accept().await?;
            let svc = handler.clone();

            // 2️⃣ Spawn one Tokio task per connection.
            tokio::spawn(async move {
                let http = Http::new();
                if let Err(err) = http.serve_connection(stream, svc).await {
                    tracing::warn!(%peer, %err, "connection error");
                }
            });
        }
    }
}