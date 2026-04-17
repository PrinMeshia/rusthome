#!/usr/bin/env bash
# §7.1 — wall-clock p95 for multi-rule property tests (oscillation + determinism proptests).
# Complements scripts/bench-p95.sh (CLI ingest micro-bench).
set -euo pipefail
RUNS="${1:-10}"
cd "$(dirname "$0")/.."
export TIMEFORMAT="%R"
times=()
for i in $(seq 1 "$RUNS"); do
  # Real time (seconds, fractional) for the full test binary including compile-free reruns.
  t=$( { time cargo test -p rusthome-app --test oscillation_proptest --test determinism_proptest --release -q 1>/dev/null 2>&1; } 2>&1 || true )
  if [[ -z "${t}" ]]; then
    echo "run $i: time parse failed" >&2
    exit 1
  fi
  # Convert to milliseconds for consistent stats with bench-p95.sh (integer ms).
  ms=$(awk -v x="$t" 'BEGIN { printf "%.0f\n", x * 1000 }')
  times+=("$ms")
  echo "run $i: ${ms}ms (${t}s)"
done
printf '%s\n' "${times[@]}" | sort -n | awk -v n="$RUNS" '
  { a[NR]=$1 }
  END {
    med = a[int((n + 1) / 2)]
    p95i = int(0.95 * n + 0.999)
    if (p95i < 1) p95i = 1
    if (p95i > n) p95i = n
    print "runs=" n ", min_ms=" a[1] ", median_ms=" med ", p95_ms=" a[p95i] ", max_ms=" a[n]
  }'
