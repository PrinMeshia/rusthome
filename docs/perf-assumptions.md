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
# Multiple runs + median / min / max (bash, GNU grep `-P`):
bash scripts/bench-p95.sh 10 200
```

Each iteration uses room `bench-{i}` to avoid `LightAlreadyOn` on the same growing journal (V0 rules turn on a light per motion).

Record `elapsed_ms` on the target (e.g. Raspberry Pi) and update this file with an order of magnitude; compare to `max_wall_ms_per_run` in `RunLimits`.

## Reference measurement (lab example)

| Command | Reported result | Derived |
|---------|-------------------|---------|
| `rusthome bench --count 50`, 2026-04-10 | `elapsed_ms = 135` (`bench_emit count=50`) | ~2.7 ms per full ingest, ~370 ingest/s; journal accumulates 50 cascades (`bench-0` … `bench-49`) |
| `rusthome bench --count 200`, 2026-04-10 | `elapsed_ms = 3105` (`bench_emit count=200`) | ~15.5 ms per full ingest, ~64 ingest/s; journal accumulates 200 cascades (rooms `bench-0` … `bench-199`) |
| `rusthome bench --count 200` (release) | `elapsed_ms ≈ 1219` | ~6.1 ms per full ingest, ~164 ingest/s; other lab machine |
| `rusthome bench --count 50` (release), 2026-04-09 | `elapsed_ms = 394` | ~7.9 ms / ingest, ~127 ingest/s; Raspberry Pi 4 Model B Rev 1.4 (`aarch64`), journal accumulates 50 cascades |

`elapsed_ms / count` is a **coarse average**: each bench iteration replays the **entire** journal so far, so later iterations cost more — comparing `count=50` vs `count=200` on the same machine is not linear in `count`.

Copy with **exact machine model**, date, and `git` revision for each row. Spread between rows is normal (CPU model, thermals, `debug` vs `--release`, background load). **p95** under real load still needs `scripts/bench-p95.sh` (multiple runs).
