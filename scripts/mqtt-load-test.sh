#!/usr/bin/env bash
# Publish N motion events via MQTT and measure end-to-end ingestion time.
# Usage: bash scripts/mqtt-load-test.sh [COUNT] [DATA_DIR]
set -euo pipefail

COUNT="${1:-50}"
DATA_DIR="${2:-data-perf-test}"
JOURNAL="${DATA_DIR}/events.jsonl"
TIMEOUT_SEC=30

lines_before=0
if [ -f "$JOURNAL" ]; then
  lines_before=$(wc -l < "$JOURNAL")
fi
expected_lines=$((lines_before + COUNT * 7))

echo "Publishing ${COUNT} motion events via MQTT (distinct rooms)..."
start_ms=$(date +%s%3N)

for i in $(seq 0 $((COUNT - 1))); do
  mosquitto_pub -t "sensors/motion/perf-room-${i}" -m "perf-room-${i}"
done

pub_ms=$(date +%s%3N)
echo "All publishes sent in $((pub_ms - start_ms))ms. Waiting for journal (${expected_lines} lines)..."

deadline=$((SECONDS + TIMEOUT_SEC))
while [ "$SECONDS" -lt "$deadline" ]; do
  current=0
  if [ -f "$JOURNAL" ]; then
    current=$(wc -l < "$JOURNAL")
  fi
  if [ "$current" -ge "$expected_lines" ]; then
    break
  fi
  sleep 0.1
done

end_ms=$(date +%s%3N)
elapsed=$((end_ms - start_ms))
pub_elapsed=$((pub_ms - start_ms))
ingest_elapsed=$((end_ms - pub_ms))
per_event=$((elapsed / COUNT))

final_lines=0
if [ -f "$JOURNAL" ]; then
  final_lines=$(wc -l < "$JOURNAL")
fi

echo "---"
echo "count=${COUNT}"
echo "total_ms=${elapsed}"
echo "publish_ms=${pub_elapsed}"
echo "ingest_tail_ms=${ingest_elapsed}"
echo "per_event_ms=${per_event}"
echo "journal_lines=${final_lines}"
echo "expected_lines=${expected_lines}"
