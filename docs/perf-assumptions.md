# Performance assumptions (plan §7.1)

See [implementation.md](implementation.md) for the current technical scope of the binary and crates.

Target hardware: Raspberry Pi class (lab reference).

## Load hypothesis (order of magnitude)

- **Nominal**: fewer than 5 external events per second.
- **Peak**: fewer than 20 external events per second.

Calibrate against real sensors and rule set before production.

## Wall-clock SLO

- **p95** processing time for a single root event run (FIFO drained or limits hit) should stay within **`max_wall_ms_per_run`** configured in `rusthome_app::RunLimits` (default 30_000 ms). Document the production value and keep it consistent with this SLO.

Logical timestamps in the journal do **not** measure real-world latency between cascade steps (plan §3.7).

## Saturation

If load exceeds the hypothesis, the runner may hit: timestamp monotonicity rejects (§3.4), `max_pending_events`, `max_events_per_run`, or `max_events_generated_per_root`. Each case returns a typed error (V0 fail-fast). Upstream backpressure is out of core scope.

## Repeatable check

```bash
cargo run -p rusthome-cli --release -- bench --count 200
# Multiple runs + min / median / p95 / max (bash, GNU grep `-P`):
bash scripts/bench-p95.sh 10 200
```

Each iteration uses room `bench-{i}` to avoid `LightAlreadyOn` on the same growing journal (V0 rules turn on a light per motion). The script prints **p95** of `elapsed_ms` across runs (wall-clock for the whole bench command, not a single event).

Record `elapsed_ms` on the target (e.g. Raspberry Pi) and update this file with an order of magnitude; compare to `max_wall_ms_per_run` in `RunLimits`.

## Reference measurement (lab example)

| Command | Reported result | Derived |
|---------|-------------------|---------|
| `rusthome bench --count 50`, 2026-04-10 | `elapsed_ms = 135` (`bench_emit count=50`) | ~2.7 ms per full ingest, ~370 ingest/s; journal accumulates 50 cascades (`bench-0` … `bench-49`) |
| `rusthome bench --count 200`, 2026-04-10 | `elapsed_ms = 3105` (`bench_emit count=200`) | ~15.5 ms per full ingest, ~64 ingest/s; journal accumulates 200 cascades (rooms `bench-0` … `bench-199`) |
| `rusthome bench --count 200` (release) | `elapsed_ms ≈ 1219` | ~6.1 ms per full ingest, ~164 ingest/s; other lab machine |
| `rusthome bench --count 50` (release), 2026-04-09 | `elapsed_ms = 394` | ~7.9 ms / ingest, ~127 ingest/s; Raspberry Pi 4 Model B Rev 1.4 (`aarch64`), journal accumulates 50 cascades |

`elapsed_ms / count` is a **coarse average**: each bench iteration replays the **entire** journal so far, so later iterations cost more — comparing `count=50` vs `count=200` on the same machine is not linear in `count`.

Copy with **exact machine model**, date, and `git` revision for each row. Spread between rows is normal (CPU model, thermals, `debug` vs `--release`, background load).

## p95 measurement (`bench-p95.sh`)

Raspberry Pi 4 Model B Rev 1.4 (`aarch64`), release build, `20ba72a`, 2026-04-15.

| Bench | Runs | Min | Median | p95 | Max | Per ingest (coarse avg) |
|-------|------|-----|--------|-----|-----|-------------------------|
| `count=50` | 10 | 128 ms | 132 ms | **180 ms** | 180 ms | ~2.6 ms |
| `count=200` | 10 | 3001 ms | 3032 ms | **3140 ms** | 3140 ms | ~15.2 ms |

The p95 for `count=200` (3140 ms) is well within the `max_wall_ms_per_run` default (30 000 ms). The per-ingest cost grows with journal size because each bench iteration replays the full journal before appending; this is expected and does not reflect steady-state single-event latency.

**Conclusion**: at `count=200` on a Pi 4, the benchmark p95 stays under 4 s, roughly 10× below the default budget. No adjustment to `RunLimits::max_wall_ms_per_run` is needed for this workload.

### Spot check (2026-04-17, informal)

Single release run on the lab Pi after routine development: `rusthome bench --count 50` reported `elapsed_ms=139`. This is **not** a p95 (one sample only); use `scripts/bench-p95.sh` for proper statistics.

## MQTT end-to-end p95 (`mqtt-p95.sh`)

Raspberry Pi 4 Model B Rev 1.4 (`aarch64`), release build, Mosquitto 2.0 local broker, 2026-04-15.

Each run: fresh journal, fresh adapter process, 20 distinct-room `MotionDetected` events published via `mosquitto_pub`. Measures wall-clock from first publish to last journal line committed.

```bash
bash scripts/mqtt-p95.sh 10 20
```

| Runs | Events/run | Min | Median | p95 | Max | Per event (median) | Per event (p95) |
|------|-----------|-----|--------|-----|-----|--------------------|-----------------|
| 10 | 20 | 145 ms | 147 ms | **214 ms** | 214 ms | ~7 ms | ~10 ms |

At ~7 ms per event (median), the adapter sustains **~140 events/s** — well above the peak hypothesis (20 events/s). The p95 per-event latency (10 ms) stays 3 orders of magnitude below `max_wall_ms_per_run`.

The MQTT path adds minimal overhead vs the synthetic bench: the per-event cost is comparable to `bench --count 50` on the same hardware, confirming that network transport (local Mosquitto) is not a bottleneck.

## Multi-rule property tests (wall-clock p95)

The §6.18 oscillation and determinism proptests exercise a **richer rule graph** (deep cascade, `RunLimits` including `max_pending_events`) than the CLI `bench` subcommand. They are not a substitute for ingest throughput measurement, but they are useful **regression guards** on the Pi: if the suite suddenly slows, investigate rule or pipeline changes.

```bash
bash scripts/proptest-suite-p95.sh 10
```

This runs `cargo test -p rusthome-app --test oscillation_proptest --test determinism_proptest --release` repeatedly and prints min / median / p95 / max of **total wall time** per run (seconds of real time, reported as integer ms). Record **machine model**, **date**, and **`git rev-parse HEAD`** when adding a row to the table below.

| Git revision | Hardware | Runs | Median (ms) | p95 (ms) | Notes |
|--------------|----------|------|-------------|----------|-------|
| *(fill on next Pi run)* | Raspberry Pi 4 (`aarch64`) | 10 | | | After changes to `crates/app/src/pipeline.rs` or `crates/app/tests/oscillation_proptest.rs` |
| `main` @ 2026-04-17 | Raspberry Pi (`aarch64`) | 5 | 938 | 1013 | `proptest-suite-p95.sh` after `rusthome-web` static CSS/JS split (informal; prefer 10+ runs for baselines) |
