#!/usr/bin/env bash
# Start mock backends for the Rust load balancer
set -Eeuo pipefail

DEFAULT_PORTS="8001,8002,8003"
DEFAULT_LOG_DIR="run_backends/$(date +%Y%m%d_%H%M%S)"

PORTS="${1:-${PORTS:-$DEFAULT_PORTS}}"
LOG_DIR="${LOG_DIR:-$DEFAULT_LOG_DIR}"

BASE_DELAY_MS="${BASE_DELAY_MS:-0}"
JITTER_MS="${JITTER_MS:-0}"
FAIL_PCT="${FAIL_PCT:-0}"

mkdir -p "$LOG_DIR"
PORTS="${PORTS//,/ }"
read -r -a PORT_ARRAY <<< "$PORTS"

echo "Starting backends on: ${PORT_ARRAY[*]}  (logs -> $LOG_DIR)"

declare -a CHILD_PIDS=()

# Helper: launch a backend, optionally under setsid
launch_backend() {
  local port="$1"
  local name="be-$port"

  local -a cmd=(cargo run --quiet --example test_backend -- "$port" "$name")

  echo "  http://127.0.0.1:$port  (pid → soon)"

  if command -v setsid >/dev/null 2>&1; then
    # Linux path: put backend in its own session/process-group
    setsid BASE_DELAY_MS="$BASE_DELAY_MS" \
           JITTER_MS="$JITTER_MS" \
           FAIL_PCT="$FAIL_PCT" \
           BACKEND_NAME="$name" \
           "${cmd[@]}" >"$LOG_DIR/$port.log" 2>&1 &
  else
    # macOS path: normal &
    BASE_DELAY_MS="$BASE_DELAY_MS" \
    JITTER_MS="$JITTER_MS" \
    FAIL_PCT="$FAIL_PCT" \
    BACKEND_NAME="$name" \
      "${cmd[@]}" >"$LOG_DIR/$port.log" 2>&1 &
  fi

  local pid=$!
  CHILD_PIDS+=("$pid")
  echo -e "\e[2A\e[0K  http://127.0.0.1:$port  (pid $pid)"   # overwrite previous line
  sleep 0.1
}

for p in "${PORT_ARRAY[@]}"; do launch_backend "$p"; done

# ── Cleanup ──────────────────────────────────────────────
cleanup() {
  echo -e "\n⏹  Stopping backends…"
  for pid in "${CHILD_PIDS[@]}"; do
    # Kill process-group if we used setsid; else just the pid
    if kill -0 "$pid" 2>/dev/null; then
      if command -v setsid >/dev/null 2>&1; then
        kill -TERM -- -"$pid" 2>/dev/null || true
      else
        kill -TERM "$pid"         2>/dev/null || true
      fi
    fi
  done

  sleep 2   # polite grace period

  for pid in "${CHILD_PIDS[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
      echo "  ⚠️  PID $pid still alive → SIGKILL"
      if command -v setsid >/dev/null 2>&1; then
        kill -KILL -- -"$pid" 2>/dev/null || true
      else
        kill -KILL "$pid"        2>/dev/null || true
      fi
    fi
  done
  echo "✔  All backends stopped."
}

trap cleanup INT TERM EXIT

echo "▶  Press Ctrl-C to stop all backends"
wait   # Wait for children (cleanup runs on Ctrl-C / exit)
