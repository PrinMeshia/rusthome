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
| **Library** `rusthome_app::rusthome_file` | Load / validate `{data-dir}/rusthome.toml` — same helpers as the CLI (`load_rusthome_file`, `resolve_rules_preset`, `build_runtime_config`, `build_run_limits`) |
| Example [`mqtt_motion_ingest`](../crates/app/examples/mqtt_motion_ingest.rs) | MQTT subscriber → `ingest_observation_with_causal` (`MotionDetected`); `rumqttc` with `default-features = false` (plain TCP) |

## Example binaries (templates)

### Observed light ([`append_observed.rs`](../crates/app/examples/append_observed.rs))

1. `rusthome_file::load_rusthome_file` + `resolve_rules_preset` / `build_runtime_config` / `build_run_limits` (same merge order as CLI)
2. `Journal::open` + `replay_state` on `data-dir/events.jsonl`
3. `append_observed_light_fact` with **Observed** `LightOn` / `LightOff`

```bash
cargo run -p rusthome-app --example append_observed -- \
  --data-dir data --timestamp 100 --room hall --state off
```

Optional `--rules-preset v0` overrides the file; default preset is `v0` if the file omits `rules_preset`.

### Turn-off command ([`ingest_turn_off.rs`](../crates/app/examples/ingest_turn_off.rs))

1. Same `rusthome.toml` loading as above
2. `ingest_command_with_causal` with `CommandEvent::TurnOffLight` (optional `--command-id` / `--causal-chain-id` UUID strings, like CLI `turn-off-light`)

```bash
cargo run -p rusthome-app --example ingest_turn_off -- \
  --data-dir data --timestamp 200 --room hall
```

### MQTT motion ([`mqtt_motion_ingest.rs`](../crates/app/examples/mqtt_motion_ingest.rs))

1. Same `rusthome.toml` loading and journal replay as the other examples
2. Subscribes with **`rumqttc`** (TCP; optional `--mqtt-user` / `--mqtt-password`)
3. Each publish → `ingest_observation_with_causal` with `ObservationEvent::MotionDetected` and a fresh `causal_chain_id` (UUID)

Payload: plain UTF-8 room name, or JSON `{"room":"…"}`; optional `"ts"` for logical time (otherwise wall ms, strictly increasing vs the last ingested event). If JSON has no `room`, the last topic segment is used when it looks like a name.

```bash
cargo run -p rusthome-app --example mqtt_motion_ingest -- \
  --data-dir data --broker 127.0.0.1 --port 1883 --topic 'sensors/motion/#'
```

Extend these with your transport; keep **domain logic** in rules, **I/O** in the adapter.

## Config parity with CLI

Examples and custom adapters should call **`rusthome_app::rusthome_file`** (same types as `crates/cli/src/config.rs`, which re-exports this module). That loads `rusthome.toml` when present and merges `physical_projection_mode`, `io_timeout_logical_delta`, `[run_limits]`, with `--rules-preset` / `--io-anchored` overrides matching the CLI.

## Web dashboard (lab)

`rusthome serve` (or the `rusthome-web` binary) replays the same `events.jsonl` as `rusthome state` and serves a minimal HTML page plus JSON APIs. Use the **same `--data-dir`** as other subcommands. See [implementation.md — rusthome-web](implementation.md#rusthome-web-read-only-ui).

There is **no authentication**. Keep the default **`127.0.0.1`** bind for local use; if you listen on all interfaces or a LAN IP, put **TLS + access control** (reverse proxy) in front — the HTML pages show a warning when the bind is not loopback-only. Step-by-step: [web-proxy.md](web-proxy.md).

## Truth and coarse state

Observed on/off does not encode *why* the lamp is off (burnt bulb, breaker, command). See [reconciliation.md](reconciliation.md) for provenance and V0 limits.
