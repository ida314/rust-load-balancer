# Tier 1 Implementation Plan — Build, Demo & CI

> **For a fresh Claude Code session:** read this file, then begin implementation of the
> deliverables in order (§1 → §7). All codebase facts needed are baked in below (§"Discovered
> facts") so you should not need to re-explore before starting. Do a clean `cargo build` first to
> see the actual warning list. This is a working doc — once Tier 1 lands it can be deleted;
> deferred work lives in the committed `ROADMAP.md` that this plan creates.

## Context

`rust-load-balancer` is a working HTTP reverse-proxy/load balancer in Rust (hyper 0.14 + tokio)
with health checks, per-backend circuit breakers, retry/backoff, connection limiting, and
Prometheus metrics. After ~11 months idle the code is sound, but **nothing packages, verifies, or
demos it**: no committed lockfile, no Dockerfile, no docker-compose, no CI. A reviewer can't
build it, can't see it run, and gets no signal it's tested.

**Goal (Tier 1 only):** a green-CI, one-command-runnable project a recruiter/engineer can
`compose up` and watch balance live traffic with a Grafana dashboard. Load-balancing algorithms
are **left as-is** (weighted/least-conn stay stubbed). Tiers 2 & 3 are deferred into a committed
`ROADMAP.md`, not implemented here.

**Environment (verified 2026-07-13):** `cargo`/`rustc` 1.96.1 present (Fedora system package).
Container runtime is **Podman 5.8.3** — no Docker. Artifacts are Docker-format for portability;
run locally with `podman compose` (reads the same files). Host arch is aarch64 (arm64) — all base
images below have arm64 variants.

---

## Deliverables (implement in this order)

### 1. Clean, reproducible build + committed lockfile
- Edit `.gitignore`: remove the `/Cargo.lock` line so the lockfile is committed (a binary/app
  must pin deps for reproducible builds).
- Get `cargo build --release` and `cargo test` green on the current toolchain, then commit
  `Cargo.lock`.
- Get `cargo clippy --all-targets` and `cargo fmt --check` clean so CI can enforce them. Known
  cleanup:
  - Delete `src/load_balancer/algorithm.rs` — it's an orphaned duplicate of `traits.rs` and is
    **not** in the module tree (`src/load_balancer/mod.rs` declares only `round_robin` and
    `traits`).
  - `shutdown_signal()` at `src/main.rs:139` is defined but unused — **wire it into the server's
    graceful shutdown** (it's a real feature and cheap) rather than deleting. If wiring proves
    fiddly with the current `ServerBuilder`, deleting is acceptable; note the choice.
  - If clippy surfaces broad churn beyond the above, set CI clippy to `-W` (warn) instead of
    `-D warnings` rather than making invasive edits. Record the decision in the PR/commit.

### 2. Dockerfile (multi-stage, layer-cached) + `.dockerignore`
New `Dockerfile`:
- **Builder** `rust:1-slim`: `apt-get install -y libssl-dev pkg-config` (reqwest 0.11 /
  hyper-tls use native-tls → OpenSSL required at build). COPY `Cargo.toml` + `Cargo.lock` first
  and pre-build deps for caching, then COPY `src/` + `examples/` and run
  `cargo build --release` **and** `cargo build --release --example test_backend`.
- **Runtime** `debian:bookworm-slim`: `apt-get install -y ca-certificates libssl3`; COPY both
  `target/release/rust-load-balancer` and `target/release/examples/test_backend` into `/usr/local/bin`;
  COPY config; `EXPOSE 8080 9090`; `CMD ["rust-load-balancer","config.yaml"]`.
- `.dockerignore`: `target/`, `.git/`, `run_*/`, `*.log`, `IMPLEMENTATION_PLAN.md`.

### 3. Required source fix for the demo
- `examples/test_backend.rs:153` binds `127.0.0.1` (loopback only) → change to `0.0.0.0` so
  backends are reachable across compose services. Correctness fix for any non-local deploy, not an
  algorithm change. Port still from argv[1], name from argv[2].

### 4. docker-compose demo stack
New `docker-compose.yml` (Compose spec; runs under `podman compose`):
- `backend-8001` / `backend-8002` / `backend-8003`: same built image, each
  `command: ["test_backend","<port>","backend-<port>"]` so service name = backend ID = matches
  `config.yaml` IDs. Optionally set `FAIL_PCT` / `BASE_DELAY_MS` env to make the demo lively.
- `loadbalancer`: same image, `command: ["rust-load-balancer","config.docker.yaml"]`, publishes
  `8080:8080` and `9090:9090`, `depends_on` the backends,
  `environment: RUST_LOG=rust_load_balancer=info,hyper=warn`.
- `prometheus` (`prom/prometheus`): mounts `deploy/prometheus.yml`, publish `9091:9090` on the
  host (the LB already uses host 9090 for its own metrics — avoid the collision).
- `grafana` (`grafana/grafana`): `3000:3000`, provisioned datasource + dashboard,
  `GF_AUTH_ANONYMOUS_ENABLED=true` and `GF_AUTH_ANONYMOUS_ORG_ROLE=Admin` for a login-free demo.
- New `config.docker.yaml`: same shape as `config.yaml` but backend URLs use compose service DNS
  (`http://backend-8001:8001`, `http://backend-8002:8002`, `http://backend-8003:8003`).

### 5. Prometheus + Grafana provisioning
- `deploy/prometheus.yml`: one scrape job `loadbalancer` → target `loadbalancer:9090`,
  `metrics_path: /metrics`, `scrape_interval: 5s`.
- `deploy/grafana/provisioning/datasources/prometheus.yml`: Prometheus datasource →
  `http://prometheus:9090`, set as default.
- `deploy/grafana/provisioning/dashboards/dashboard.yml` (provider) + a dashboard JSON with
  panels built on the **actual exported metrics** (see Discovered facts):
  - Request rate: `sum by (backend) (rate(lb_requests_total[1m]))`
  - p95 latency: `histogram_quantile(0.95, sum by (le) (rate(lb_request_duration_seconds_bucket[5m])))`
  - Backend health: `lb_backend_health_status`
  - Active connections: `lb_active_connections`
  - Circuit breaker state: `lb_circuit_breaker_state`
  - Healthy vs total: `lb_healthy_backends`, `lb_total_backends`

### 6. GitHub Actions CI
New `.github/workflows/ci.yml`, on push + PR to `main`:
- `actions/checkout`, `dtolnay/rust-toolchain@stable` (components `rustfmt`, `clippy`),
  `Swatinem/rust-cache`.
- Steps: `cargo fmt --all --check` → `cargo clippy --all-targets -- -D warnings` (or `-W` per §1)
  → `cargo test --all` → `cargo build --release`.
  (`cargo test` uses the dev/test profile, so `panic = "abort"` in `[profile.release]` does not
  block unwinding in tests.)
- Add the CI status badge to the top of `README.md`.

### 7. README + ROADMAP
- `README.md`: add a **Quick Start** — `podman compose up --build` (note `docker compose` is
  identical), then `curl http://localhost:8080/`, metrics `:9090/metrics`, Grafana `:3000`
  (anonymous). Add CI badge. Remove the stale "see the example in the artifacts" line (~README:82).
- New committed `ROADMAP.md` with the deferred work:
  - **Tier 2 (depth):** implement weighted round-robin + least-connections — replace the
    placeholder match arms in `src/load_balancer/mod.rs:12` (the default config already asks for
    `weighted_round_robin` but silently runs plain RR); wire graceful shutdown if not done in §1;
    fill the empty async test stubs in `tests/load_balancer_tests.rs`; record real benchmark
    numbers + a chart (finish `scripts/visualize_benchmarks.py`, currently a non-functional stub)
    and reconcile `BENCHMARKS.md` (it references scripts that don't exist: `./run_benchmarks.sh`,
    `benchmarks/memory_profile.sh`, `benchmarks/connection_pooling.sh`).
  - **Tier 3 (differentiators):** token-bucket rate limiting; config hot-reload via `arc-swap`
    (already a dependency); hyper 0.14 → 1.0 / axum migration; distributed tracing.

---

## Files touched
- **New:** `Dockerfile`, `.dockerignore`, `docker-compose.yml`, `config.docker.yaml`,
  `deploy/prometheus.yml`, `deploy/grafana/provisioning/**`, `.github/workflows/ci.yml`,
  `ROADMAP.md`, committed `Cargo.lock`.
- **Edit:** `.gitignore` (unignore Cargo.lock), `examples/test_backend.rs` (bind `0.0.0.0`),
  `README.md` (quick start + badge), `src/main.rs` (graceful shutdown; warning cleanup).
- **Delete:** `src/load_balancer/algorithm.rs`.

---

## Verification
1. **Local (mirrors CI):** `cargo build --release`, `cargo test`, `cargo clippy --all-targets`,
   `cargo fmt --check` — all green.
2. **Demo stack:** `podman compose up --build`, then:
   - `for i in $(seq 6); do curl -s -D- http://localhost:8080/ -o /dev/null | grep -i x-backend-id; done`
     → requests rotate across `backend-8001/8002/8003`.
   - `curl -s "http://localhost:8080/bytes?size=2048" | wc -c` → ~2048 (proxy path + example endpoint work).
   - `curl -s http://localhost:9090/metrics | grep lb_requests_total` → metrics populated.
   - Open `http://localhost:3000` → dashboard renders request rate / health / latency.
   - `podman compose stop backend-8002`; after ~15s `lb_backend_health_status{backend="backend-8002"}`
     flips to 0 and traffic shifts → live health-check failover.
3. **CI:** validate the workflow file; the green check lands on first push (pushing is out of
   scope unless the user asks).

---

## Decisions (reversible, pre-made)
- Container files are **Docker-format** (portable / resume-standard) but verified locally with
  **Podman** since Docker isn't installed.
- Grafana **anonymous access on** for a zero-friction local demo; documented as local-only.
- Weighted/least-conn balancing stays **stubbed** (Tier 2). The live demo therefore shows ~even
  round-robin; README/ROADMAP state this honestly instead of implying weights work.

---

## Discovered facts (baked in — no re-exploration needed)

**Ports / entrypoints**
- LB main proxy: `0.0.0.0:8080`, **hardcoded** at `src/main.rs:68` (not config/env-driven).
- LB metrics server: `0.0.0.0:<config.metrics.port>` path `<config.metrics.path>`; defaults
  9090 + `/metrics` (`src/config/models.rs:186-188`, `config.yaml:35-38`). Separate hyper server
  (`src/main.rs:79-126`).
- Config path: positional **argv[1]**, default `config.yaml` in CWD (`src/main.rs:38`). No env/flag.
- Binary name: `rust-load-balancer` (`Cargo.toml`, no `[[bin]]`) → `target/release/rust-load-balancer`.
  Example → `target/release/examples/test_backend` (only built with `--example test_backend`).
- Only env var the LB itself reads: `RUST_LOG` (via `tracing_subscriber` `from_default_env`,
  `src/main.rs:31`).

**`examples/test_backend.rs`**
- Port = argv[1] (default 8001); name = argv[2] / `BACKEND_NAME` / `backend-<port>`.
- **Binds `127.0.0.1` at line 153 — must change to `0.0.0.0` for compose (§3).**
- Endpoints: `/health` (200 `OK` / 503), `/echo` (JSON + `X-Backend-Name` header),
  `/bytes?size=N` (N bytes, default 1024), any other path → JSON with optional simulated delay.
- Behavior env: `BASE_DELAY_MS`, `JITTER_MS`, `FAIL_PCT` (all default 0).

**Exported Prometheus metrics** (`src/metrics/collector.rs`)
- `lb_requests_total` {method,status_code,backend}; `lb_request_duration_seconds`
  {method,status_code,backend}; `lb_request_size_bytes` {method}; `lb_response_size_bytes`
  {method,status_code}.
- `lb_backend_requests_total` {backend,status}; `lb_backend_request_duration_seconds` {backend};
  `lb_backend_connections_active` {backend}; `lb_backend_health_status` {backend} (1/0).
- `lb_circuit_breaker_state` {backend} (0 closed / 1 open / 2 half-open);
  `lb_circuit_breaker_failures_total` {backend}.
- `lb_active_connections`; `lb_healthy_backends`; `lb_total_backends` (unlabeled gauges).

**Build/CI notes**
- `edition = "2021"`, no MSRV pin. Key deps: tokio 1, hyper 0.14, reqwest 0.11 (native-tls),
  prometheus 0.13, serde_yaml 0.9, dashmap 5.5, arc-swap 1.6.
- `[profile.release]`: `lto = true`, `codegen-units = 1`, `panic = "abort"`, `opt-level = 3` →
  slow release builds; use multi-stage Docker dep caching. `panic = "abort"` is release-only, so
  `cargo test` (dev/test profile) is unaffected.
- **No existing** Dockerfile, compose, prometheus/grafana config, `.github/`, or `Cargo.lock`.
- Tests: `tests/load_balancer_tests.rs` = 3 empty async stubs (compiles, asserts nothing);
  `tests/integration_test.sh` = incomplete bash (needs `jq`, `lsof`). CI `cargo test` passes
  trivially today.
