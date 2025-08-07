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

// ——————————————————————————————————————————
// Main
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Parse args / env ─────────────────────────────────────────────
    let port: u16 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "8001".into())
        .parse()?;
    let name = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("BACKEND_NAME").ok())
        .unwrap_or_else(|| format!("backend-{port}"));

    let base_delay =
        std::env::var("BASE_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let jitter_ms =
        std::env::var("JITTER_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let fail_pct =
        std::env::var("FAIL_PCT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);

    // ── Shared state ────────────────────────────────────────────────
    let state = BackendState {
        port,
        name: name.clone(),
        req_counter: Arc::new(AtomicU64::new(0)),
        healthy_flag: Arc::new(AtomicBool::new(true)),
        base_delay,
        jitter_ms,
        fail_pct,
    };

    // Toggle health every 30 s (demo)
    {
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(30)).await;
                let cur = st.healthy_flag.load(Ordering::SeqCst);
                st.healthy_flag.store(!cur, Ordering::SeqCst);
                println!(
                    "[{}] Health flipped → {}",
                    st.name,
                    if !cur { "healthy" } else { "unhealthy" }
                );
            }
        });
    }

    // ── Hyper server ────────────────────────────────────────────────
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let make_svc = make_service_fn(move |_conn| {
        let st = state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle(req, st.clone())
            }))
        }
    });

    println!(
        "Mock backend '{}' on http://{}  [delay={}ms±{} fail={} %]",
        name, addr, base_delay, jitter_ms, fail_pct
    );

    Server::bind(&addr).serve(make_svc).await?;
    Ok(())
}
