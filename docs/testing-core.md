# Core engine regression (§6.18 / plan Phase 1)

Use this checklist **before** extending event types, rule graphs, or `RunLimits` defaults.

## Determinism and oscillation (property tests)

| Test binary | Role |
| ----------- | ---- |
| [`crates/app/tests/determinism_proptest.rs`](../crates/app/tests/determinism_proptest.rs) | Same synthetic sequences → byte-identical journal and state; includes §6.6 cap paths |
| [`crates/app/tests/oscillation_proptest.rs`](../crates/app/tests/oscillation_proptest.rs) | Deep cascade / `max_pending_events` — regression guard for rule graph + FIFO |

Run once (debug or release):

```bash
cargo test -p rusthome-app --test determinism_proptest --test oscillation_proptest --release
```

## Wall-clock p95 of the property suite (Pi / lab)

The CLI `bench` subcommand does **not** replace these tests: it measures ingest throughput on a growing journal, not multi-rule oscillation.

```bash
bash scripts/proptest-suite-p95.sh 10
```

Records **total wall time** per full `cargo test` invocation (compile-free reruns are still faster). Add a row to [perf-assumptions.md](perf-assumptions.md) when changing `pipeline`, rules, or proptest configs.

## MQTT ↔ journal integration (spot checks)

| Test binary | Role |
| ----------- | ---- |
| [`crates/app/tests/mqtt_command_ingest.rs`](../crates/app/tests/mqtt_command_ingest.rs) | `commands/light/...` → `TurnOnLight` / `TurnOffLight` |
| [`crates/app/tests/mqtt_observation_closed_loop.rs`](../crates/app/tests/mqtt_observation_closed_loop.rs) | `sensors/motion/...` → rules → derived light state; replay |

```bash
cargo test -p rusthome-app --test mqtt_command_ingest --test mqtt_observation_closed_loop --release
```

## Synthetic ingest throughput

```bash
bash scripts/bench-p95.sh 10 200
```

See [perf-assumptions.md](perf-assumptions.md) for SLO interpretation and recorded Pi numbers.
