#!/usr/bin/env bash
# benchmarks/throughput.sh - Fixed version
source "$(dirname "$0")/common.sh"

SIZE=$((1024*1024)) # 1 MiB per request
URL="http://${LB_HOST}:${LB_PORT}/bytes?size=$SIZE"
DUR=30s

echo "Running throughput test..."
echo "Request size: 1 MiB"
echo "Duration: $DUR"
echo ""

# Clear metrics before test
curl -sf "$METRICS" >/dev/null

# Run the test
wrk -t4 -c64 -d"$DUR" --latency "$URL" >tp.log 2>&1

# Extract results
rps=$(grep "Requests/sec" tp.log | awk '{print $2}')
transfer=$(grep "Transfer/sec" tp.log | awk '{print $2}')

# Calculate throughput
gbps=$(awk -v r="$rps" -v s="$SIZE" 'BEGIN{printf "%.2f", r*s/1024/1024/1024}')

echo "Results:"
echo "Requests/sec: $rps"
echo "Transfer/sec: $transfer"
echo "Calculated throughput: $gbps GB/s"

# Get total bytes from Prometheus
bytes_total=$(scrape 'lb_response_size_bytes_sum' || echo "0")
count_total=$(scrape 'lb_response_size_bytes_count' || echo "0")

if [[ "$bytes_total" != "0" ]] && [[ "$count_total" != "0" ]]; then
    avg_size=$(awk -v b="$bytes_total" -v c="$count_total" 'BEGIN{printf "%.0f", b/c}')
    echo ""
    echo "Prometheus metrics:"
    echo "Total bytes served: $bytes_total"
    echo "Total requests: $count_total"
    echo "Average response size: $avg_size bytes"
fi

rm -f tp.log