// ────────────────────────────────
// src/server/listener.rs
// Encapsulates low‑level TCP bind/accept so we can swap TLS later.
// ────────────────────────────────
use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn bind_tcp(addr: SocketAddr) -> Result<TcpListener> {
    let listener = TcpListener::bind(addr).await?;
    Ok(listener)
}
