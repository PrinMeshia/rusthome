# Edge integration (observations and adapters)

The FIFO engine does **not** read sensors or the network. A separate **adapter** process (or thread) is responsible for turning real-world signals into **journal appends**.

## Paths into the system

| Mechanism | Use case |
| --------- | -------- |
| CLI `observed-light` | Manual ops, scripts, quick tests |
| CLI `emit` | Ingest `MotionDetected` (and cascade) with explicit logical timestamp |
| CLI `turn-off-light` | Ingest `TurnOffLight` (R7); `--trace-file`; optional `--command-id` / `--causal-chain-id` (UUID) for dedup §14.3 and reproducible `explain` |
| **Library** `rusthome_app::append_observed_light_fact` | Custom code: MQTT, REST webhook, GPIO poller, … |
| **Library** `rusthome_app::ingest_observation_with_causal` | Push `Observation` events (e.g. motion) with your own `causal_chain_id` |
| **Library** `rusthome_app::ingest_command_with_causal` | Push `Command` lines (e.g. `TurnOffLight` from a switch adapter) with your own `causal_chain_id` |

## Example binaries (templates)

### Observed light ([`append_observed.rs`](../crates/app/examples/append_observed.rs))

1. `Journal::open` + `replay_state` on `data-dir/events.jsonl`
2. Load a `RulesPreset` (same rule set as the rest of the deployment)
3. `append_observed_light_fact` with **Observed** `LightOn` / `LightOff`

```bash
cargo run -p rusthome-app --example append_observed -- \
  --data-dir data --timestamp 100 --room hall --state off --rules-preset v0
```

### Turn-off command ([`ingest_turn_off.rs`](../crates/app/examples/ingest_turn_off.rs))

1. Same journal + preset setup
2. `ingest_command_with_causal` with `CommandEvent::TurnOffLight` (optional `--command-id` / `--causal-chain-id` UUID strings, like CLI `turn-off-light`)

```bash
cargo run -p rusthome-app --example ingest_turn_off -- \
  --data-dir data --timestamp 200 --room hall --rules-preset minimal
```

Extend these with your transport; keep **domain logic** in rules, **I/O** in the adapter.

## Config parity with CLI

The example accepts `--rules-preset` and `--io-anchored` only. The full CLI also merges `rusthome.toml` (`physical_projection_mode`, `io_timeout_logical_delta`, `[run_limits]`). For production adapters, either duplicate that loading (see `crates/cli/src/config.rs`) or share a small internal crate later.

## Truth and coarse state

Observed on/off does not encode *why* the lamp is off (burnt bulb, breaker, command). See [reconciliation.md](reconciliation.md) for provenance and V0 limits.
