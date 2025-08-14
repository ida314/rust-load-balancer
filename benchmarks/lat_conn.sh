#!/usr/bin/env bash
# benchmarks/lat_conn.sh - Debug version to show wrk output
set -Eeuo pipefail
source "$(dirname "$0")/common.sh"

URL="http://${LB_HOST}:${LB_PORT}/bytes?size=1024"
DUR="15s"

for conns in 100 500 1000 2000; do
    echo "============================================"
    echo "Testing with $conns connections"
    echo "============================================"
    
    # Run wrk and show output directly
    wrk -t4 -c"$conns" -d"$DUR" --latency "$URL"
    
    # Also show max connections metric
    maxconn=$(scrape lb_active_connections || true)
    echo ""
    echo "Max active connections: $maxconn"
    echo ""
    
    # Add a pause between tests
    sleep 2
done