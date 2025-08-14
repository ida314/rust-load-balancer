#!/usr/bin/env bash
# benchmarks/latency_percentiles.sh
source "$(dirname "$0")/common.sh"

URL="http://${LB_HOST}:${LB_PORT}/echo"
DURATION="60s"
CONNECTIONS="1000"

echo "Running extended latency analysis..."
echo "URL: $URL"
echo "Duration: $DURATION"
echo "Connections: $CONNECTIONS"
echo ""

# Run wrk with detailed latency output
wrk -t8 -c"$CONNECTIONS" -d"$DURATION" --latency "$URL" | tee latency_full.log

# Extract percentiles
echo ""
echo "=== Latency Percentiles ==="
grep -E "^\s*(50|75|90|95|99|99\.9|99\.99)%" latency_full.log || {
    # Fallback if detailed percentiles not available
    echo "P50:    $(grep "50%" latency_full.log | awk '{print $2}')"
    echo "P90:    $(grep "90%" latency_full.log | awk '{print $2}')"
    echo "P99:    $(grep "99%" latency_full.log | awk '{print $2}')"
}

# Calculate requests per second per core
rps=$(grep "Requests/sec" latency_full.log | awk '{print $2}')
cores=$(nproc)
rps_per_core=$(echo "$rps $cores" | awk '{printf "%.2f", $1/$2}')

echo ""
echo "=== Performance Summary ==="
echo "Total RPS: $rps"
echo "RPS per core: $rps_per_core"
echo "Active connections (peak): $(scrape lb_active_connections)"