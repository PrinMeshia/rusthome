# Edge integration (observations and adapters)

The FIFO engine does **not** read sensors or the network. A separate **adapter** process (or thread) is responsible for turning real-world signals into **journal appends**.

## Paths into the system

| Mechanism | Use case |
| --------- | -------- |
| CLI `observed-light` | Manual ops, scripts, quick tests |
| CLI `emit` | Ingest `MotionDetected` (and cascade) with explicit logical timestamp |
| **Library** `rusthome_app::append_observed_light_fact` | Custom code: MQTT, REST webhook, GPIO poller, … |
| **Library** `rusthome_app::ingest_observation_with_causal` | Push `Observation` events (e.g. motion) with your own `causal_chain_id` |

## Example binary (template)

[`crates/app/examples/append_observed.rs`](../crates/app/examples/append_observed.rs) shows the minimal flow:

1. `Journal::open` + `replay_state` on `data-dir/events.jsonl`
2. Load a `RulesPreset` (same rule set as the rest of the deployment)
3. `append_observed_light_fact` with **Observed** `LightOn` / `LightOff`

Run:

```bash
cargo run -p rusthome-app --example append_observed -- \
  --data-dir data --timestamp 100 --room hall --state off --rules-preset v0
```

Extend this example with your transport; keep **domain logic** in rules, **I/O** in the adapter.

## Config parity with CLI

The example accepts `--rules-preset` and `--io-anchored` only. The full CLI also merges `rusthome.toml` (`physical_projection_mode`, `io_timeout_logical_delta`, `[run_limits]`). For production adapters, either duplicate that loading (see `crates/cli/src/config.rs`) or share a small internal crate later.

## Truth and coarse state

Observed on/off does not encode *why* the lamp is off (burnt bulb, breaker, command). See [reconciliation.md](reconciliation.md) for provenance and V0 limits.
