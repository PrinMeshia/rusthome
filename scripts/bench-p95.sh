#!/usr/bin/env bash
# §7.1 — plusieurs runs `rusthome bench`, stats (min / median / p95 / max).
set -euo pipefail
RUNS="${1:-10}"
COUNT="${2:-200}"
cd "$(dirname "$0")/.."
times=()
for i in $(seq 1 "$RUNS"); do
  ms=$(cargo run -p rusthome-cli --release --quiet -- bench --count "$COUNT" 2>&1 \
    | grep -oP 'elapsed_ms=\K\d+' || true)
  if [[ -z "${ms}" ]]; then
    echo "run $i: parse failed" >&2
    exit 1
  fi
  times+=("$ms")
  echo "run $i: ${ms}ms"
done
printf '%s\n' "${times[@]}" | sort -n | awk -v n="$RUNS" '
  { a[NR]=$1 }
  END {
    med = a[int((n + 1) / 2)]
    p95i = int(0.95 * n + 0.999)
    if (p95i < 1) p95i = 1
    if (p95i > n) p95i = n
    print "count=" n ", min=" a[1] ", median=" med ", p95=" a[p95i] ", max=" a[n]
  }'
