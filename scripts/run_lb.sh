#!/usr/bin/env bash
# scripts/run_lb.sh — Start the Rust load balancer (and optional mock backends)
set -Eeuo pipefail

ulimit -n 10000 2>/dev/null || echo "⚠️  could not raise nofile limit"

# ──────────────────────────────
# Configurable defaults
# ──────────────────────────────
CONFIG_PATH="${CONFIG_PATH:-config.yaml}"   # passed to your main.rs as argv[1]
BACKENDS_DEFAULT="8001,8002,8003"           # used only if --start-backends is given
RELEASE_BUILD=false                         # toggle with --release
NO_BUILD=false                              # toggle with --no-build
START_BACKENDS=""                           # set by --start-backends <ports>
LOG_DIR="run_logs/$(date +%Y%m%d_%H%M%S)"
RUST_LOG_DEFAULT="${RUST_LOG:-info}"        # only used if caller hasn't set RUST_LOG

# Colors
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

usage() {
  cat <<'USAGE'
Usage: scripts/run_lb.sh [options]

Options:
  --config <path>          Path to config file (default: config.yaml)
  --start-backends [ports] Start mock backends on comma-separated ports (default: 8001,8002,8003)
  --release                Build and run in --release mode
  --no-build               Skip building (run current target)
  --log-dir <dir>          Directory for logs (default: run_logs/<timestamp>)
  --help                   Show this help

Examples:
  scripts/run_lb.sh --config config.yaml
  scripts/run_lb.sh --start-backends 7001,7002 --release
  CONFIG_PATH=deploy/prod.yaml RUST_LOG=debug scripts/run_lb.sh
USAGE
}

# ──────────────────────────────
# Arg parsing
# ──────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)        CONFIG_PATH="$2"; shift 2;;
    --start-backends)
      if [[ "${2:-}" =~ ^- ]] || [[ -z "${2:-}" ]]; then
        START_BACKENDS="$BACKENDS_DEFAULT"; shift 1
      else
        START_BACKENDS="$2"; shift 2
      fi
      ;;
    --release)       RELEASE_BUILD=true; shift 1;;
    --no-build)      NO_BUILD=true; shift 1;;
    --log-dir)       LOG_DIR="$2"; shift 2;;
    --help|-h)       usage; exit 0;;
    *) echo -e "${RED}Unknown option: $1${NC}"; usage; exit 1;;
  esac
done

# ──────────────────────────────
# Pre-flight checks
# ──────────────────────────────
command -v cargo >/dev/null || { echo -e "${RED}cargo not found. Install Rust toolchain first.${NC}"; exit 1; }
[[ -f "$CONFIG_PATH" ]] || { echo -e "${RED}Config file not found: $CONFIG_PATH${NC}"; exit 1; }

mkdir -p "$LOG_DIR"
echo -e "${GREEN}▶ Log directory:${NC} $LOG_DIR"

# Ensure RUST_LOG set (doesn't override if user already exported)
export RUST_LOG="${RUST_LOG:-$RUST_LOG_DEFAULT}"

# Track PIDs to clean up
PIDS_TO_KILL=()

cleanup() {
  local code=$?
  echo -e "${YELLOW}\n⏹ Cleaning up...${NC}"
  if [[ ${#PIDS_TO_KILL[@]} -gt 0 ]]; then
    kill "${PIDS_TO_KILL[@]}" >/dev/null 2>&1 || true
    wait "${PIDS_TO_KILL[@]}" 2>/dev/null || true
  fi
  echo -e "${GREEN}✔ Done.${NC}"
  exit $code
}
trap cleanup INT TERM EXIT

# ──────────────────────────────
# Optionally start mock backends
# ──────────────────────────────
if [[ -n "$START_BACKENDS" ]]; then
  IFS=',' read -r -a PORTS <<< "$START_BACKENDS"
  echo -e "${GREEN}▶ Starting mock backends on ports:${NC} ${PORTS[*]}"
  for port in "${PORTS[@]}"; do
    # Requires examples/test_backend.rs in your repo
    ( set -Eeuo pipefail
      cargo run ${RELEASE_BUILD:+--release} --example test_backend -- "$port" \
        > "$LOG_DIR/backend_${port}.log" 2>&1
    ) &
    PIDS_TO_KILL+=($!)
    # Small stagger to avoid all binding at once
    sleep 0.15
  done
  echo -e "${GREEN}▶ Backend logs:${NC} $LOG_DIR/backend_*.log"
fi

# ──────────────────────────────
# Build (optional) and run LB
# ──────────────────────────────
if ! $NO_BUILD; then
  echo -e "${GREEN}▶ Building load balancer (${RELEASE_BUILD:+release mode}${RELEASE_BUILD:-debug mode})...${NC}"
  cargo build ${RELEASE_BUILD:+--release}
fi

echo -e "${GREEN}▶ Running load balancer with config:${NC} $CONFIG_PATH"
echo -e "${YELLOW}  (LB listens on 0.0.0.0:8080 per current src/main.rs)${NC}"
echo -e "${GREEN}▶ Metrics:${NC} If enabled in config, check http://localhost:<metrics_port><metrics_path>"

# Run in foreground so you see logs; cleanup trap handles backends on Ctrl-C.
# Using `cargo run` ensures argv[1] = CONFIG_PATH as your main.rs expects.
cargo run ${RELEASE_BUILD:+--release} -- "$CONFIG_PATH"
