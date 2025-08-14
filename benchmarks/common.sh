#!/usr/bin/env bash
# benchmarks/common.sh  –  shared helpers for all bench scripts
set -Eeuo pipefail

###############################################################################
# 1) Cross-platform core-utils helpers
###############################################################################
OS=$(uname -s)

# GNU date on Linux, BSD date on macOS.  We need epoch-ms.
if command -v gdate >/dev/null 2>&1; then
  date_ms() { gdate +%s%3N; }          # coreutils date (brew install coreutils)
else
  # BSD date (macOS) – no %N. Use perl for millis.
  date_ms() { perl -MTime::HiRes=time -e 'printf("%.0f\n", time()*1000)' ; }
fi

# Inflight LB host/ports (override via env).
LB_HOST="${LB_HOST:-127.0.0.1}"
LB_PORT="${LB_PORT:-8080}"
METRICS_PORT="${METRICS_PORT:-9090}"
METRICS="http://${LB_HOST}:${METRICS_PORT}/metrics"

###############################################################################
# 2) scrape() – fetch one Prometheus sample
###############################################################################
# Usage: scrape METRIC_NAME [label_filter_regex]
# Example: scrape lb_active_connections
#          scrape lb_backend_health_status '{backend="backend-8001"}'
scrape() {
  local name=$1; local filter=${2-}
  # curl -fsS keeps silent on success; exits non-zero on network errors.
  curl -fsS "$METRICS" |
    ( grep -E "^${name}${filter}[[:space:]]" || true ) |
    head -n1 | awk '{print $2}'
}

###############################################################################
# 3) Port-to-PID helper (used by failover.sh) – cross-platform
###############################################################################
pid_for_port() {
  local port=$1
  if command -v lsof >/dev/null 2>&1; then
    lsof -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null | head -n1
  elif command -v ss >/dev/null 2>&1; then          # Linux minimal image
    ss -ltnp "sport = :$port" | awk 'NR==2 {split($NF,a,","); print a[2]+0}'
  else
    echo "ERR"    # should never hit
  fi
}

###############################################################################
# 4) Colour helper (optional)
###############################################################################
c_yel() { printf "\033[1;33m%s\033[0m\n" "$*"; }

###############################################################################
# 5) Sanity check – warn if metrics endpoint not reachable
###############################################################################
if ! curl -fsS "$METRICS" >/dev/null 2>&1; then
  c_yel "⚠️  Metrics endpoint $METRICS not reachable.
     Make sure the load balancer started with Prometheus metrics enabled."
fi
