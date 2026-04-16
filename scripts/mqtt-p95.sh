#!/usr/bin/env bash
# §7.1 — p95 end-to-end latency via MQTT adapter (fresh journal per run).
# Usage: bash scripts/mqtt-p95.sh [RUNS] [COUNT_PER_RUN]
set -euo pipefail

RUNS="${1:-10}"
COUNT="${2:-20}"
BASE_DIR="$(mktemp -d)"
ADAPTER="target/release/examples/mqtt_motion_ingest"
trap 'pkill -f "mqtt_motion_ingest.*p95test" 2>/dev/null; rm -rf "$BASE_DIR"' EXIT

cd "$(dirname "$0")/.."

if [ ! -x "$ADAPTER" ]; then
  echo "Build first: cargo build -p rusthome-app --example mqtt_motion_ingest --release" >&2
  exit 1
fi

times=()
for run in $(seq 1 "$RUNS"); do
  data_dir="${BASE_DIR}/run-${run}"
  mkdir -p "$data_dir"
  cp configs/rusthome.example.toml "$data_dir/rusthome.toml"
  journal="${data_dir}/events.jsonl"
  expected=$((COUNT * 7))
  topic="sensors/motion/p95test${run}"

  "$ADAPTER" --data-dir "$data_dir" --broker 127.0.0.1 --port 1883 --topic "${topic}/#" \
    >/dev/null 2>&1 &
  adapter_pid=$!
  sleep 0.5

  start_ms=$(date +%s%3N)
  for i in $(seq 0 $((COUNT - 1))); do
    mosquitto_pub -t "${topic}/room-${i}" -m "room-${i}"
  done

  deadline=$((SECONDS + 30))
  while [ "$SECONDS" -lt "$deadline" ]; do
    current=0
    if [ -f "$journal" ]; then
      current=$(wc -l < "$journal")
    fi
    if [ "$current" -ge "$expected" ]; then
      break
    fi
    sleep 0.05
  done
  end_ms=$(date +%s%3N)

  kill "$adapter_pid" 2>/dev/null
  wait "$adapter_pid" 2>/dev/null || true

  elapsed=$((end_ms - start_ms))
  per_event=$((elapsed / COUNT))
  times+=("$elapsed")
  echo "run ${run}: ${elapsed}ms total, ~${per_event}ms/event (${COUNT} events)"
done

printf '%s\n' "${times[@]}" | sort -n | awk -v n="$RUNS" -v c="$COUNT" '
  { a[NR]=$1 }
  END {
    med = a[int((n + 1) / 2)]
    p95i = int(0.95 * n + 0.999)
    if (p95i < 1) p95i = 1
    if (p95i > n) p95i = n
    per_med = int(med / c)
    per_p95 = int(a[p95i] / c)
    print "---"
    print "runs=" n ", count_per_run=" c
    print "min=" a[1] "ms, median=" med "ms, p95=" a[p95i] "ms, max=" a[n] "ms"
    print "per_event: median=~" per_med "ms, p95=~" per_p95 "ms"
  }'
