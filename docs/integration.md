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
| **Library** `rusthome_app::mqtt_ingest` | MQTT topic → [`dispatch_mqtt_publish`](../crates/app/src/mqtt_ingest.rs): observations **and** `commands/light/...` → `TurnOnLight` / `TurnOffLight`; used by `rusthome serve` and the standalone adapter |
| Example [`mqtt_motion_ingest`](../crates/app/examples/mqtt_motion_ingest.rs) | MQTT subscriber for external brokers → `dispatch_mqtt_publish` (subscribe to `sensors/#` and/or `commands/#`) |

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

### MQTT adapter ([`mqtt_motion_ingest.rs`](../crates/app/examples/mqtt_motion_ingest.rs))

Standalone adapter for connecting to an **external** MQTT broker. Uses the shared [`mqtt_ingest::dispatch_mqtt_publish`](../crates/app/src/mqtt_ingest.rs) entry point:

- **Observations**: `sensors/motion/...`, `sensors/temperature/...`, `sensors/contact/...`
- **Commands**: `commands/light/{room}/on|off` → journal `CommandEvent` (same as the embedded ingest in `rusthome serve`)

1. Same `rusthome.toml` loading and journal replay as the other examples
2. Subscribes with **`rumqttc`** (TCP; optional `--mqtt-user` / `--mqtt-password`). Use **`--topic`** for each subscription you need; for both sensors and commands, run **two** processes or extend the example to subscribe to multiple filters (e.g. `sensors/#` and `commands/#`).
3. Each publish → `dispatch_mqtt_publish`, which ingests an observation, a command, or skips unknown topics

```bash
# Observations only (typical bridge / Zigbee2MQTT prefix)
cargo run -p rusthome-app --example mqtt_motion_ingest -- \
  --data-dir data --broker 127.0.0.1 --port 1883 --topic 'sensors/#'

# Light commands from another system (e.g. Home Assistant → MQTT)
cargo run -p rusthome-app --example mqtt_motion_ingest -- \
  --data-dir data --broker 127.0.0.1 --port 1883 --topic 'commands/#'
```

For most deployments, prefer `rusthome serve` (embedded broker) instead. This example is useful when connecting to a broker you already run externally.

Extend these with your transport; keep **domain logic** in rules, **I/O** in the adapter.

## All-in-one deployment (`rusthome serve`)

`rusthome serve` runs **three components in a single process**:

1. **Embedded MQTT broker** (`rumqttd`) listening on TCP `0.0.0.0:<mqtt-port>` (default 1883)
2. **Ingest adapter** connected to the broker via an in-process link (zero-copy, no TCP loopback)
3. **Web dashboard** (Axum) on `<bind>` (default `127.0.0.1:8080`)

External sensors (Zigbee2MQTT, Tasmota, etc.) connect to the embedded broker over TCP. No external Mosquitto needed.

### Supported MQTT topics

| Pattern | Ingested as |
|---|---|
| `sensors/motion/{room}` | `ObservationEvent::MotionDetected` |
| `sensors/temperature/{sensor_id}` | `ObservationEvent::TemperatureReading` |
| `sensors/contact/{sensor_id}` | `ObservationEvent::ContactChanged` |
| `commands/light/{room}/on` | `CommandEvent::TurnOnLight` |
| `commands/light/{room}/off` | `CommandEvent::TurnOffLight` |

Payload: plain UTF-8 entity name, or JSON (see [`mqtt_ingest`](../crates/app/src/mqtt_ingest.rs) for details). Command topics ignore payload for classification (empty payload is fine).

**Subscriptions in `rusthome serve`**: the ingest link subscribes to **`--mqtt-topic`** (default `sensors/#`) **and** always subscribes to **`commands/#`**, so light commands published by the web UI or external clients are ingested without extra flags.

### Web UI → MQTT → journal

When the embedded broker is active, the dashboard can POST to **`/api/command`** with JSON `{"action":"turn_on"|"turn_off","room":"<room>"}`. The server publishes to `commands/light/<room>/on` or `.../off`; the ingest loop receives those publishes and appends the corresponding command lines. If the broker is disabled (`--no-broker`), `POST /api/command` returns **503**.

### CLI flags

```text
rusthome serve [OPTIONS]
  --bind <ADDR>        Web dashboard bind address  [default: 127.0.0.1:8080]
  --mqtt-port <PORT>   Embedded broker TCP port     [default: 1883]
  --mqtt-topic <TOPIC> Ingest topic filter          [default: sensors/#]
  --no-broker          Disable broker (web only, legacy mode)
```

### Running as a systemd service

Unit file: [`configs/rusthome.service`](../configs/rusthome.service).

```bash
cargo build -p rusthome-cli --release

sudo cp configs/rusthome.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now rusthome

# Check status / logs
systemctl status rusthome
journalctl -u rusthome -f
```

Edit `ExecStart` in the unit file to adjust `--bind`, `--mqtt-port`, `--mqtt-topic`, or `--data-dir`.

### Legacy: separate services (external broker)

For setups using an **external** MQTT broker (e.g. Mosquitto), the split service files are still available:

| File | Service |
| ---- | ------- |
| [`rusthome-mqtt.service`](../configs/rusthome-mqtt.service) | MQTT adapter (connects to external broker) |
| [`rusthome-web.service`](../configs/rusthome-web.service) | Web dashboard only (`--no-broker` equivalent) |

See the files for installation instructions.

## Config parity with CLI

Examples and custom adapters should call **`rusthome_app::rusthome_file`** (same types as `crates/cli/src/config.rs`, which re-exports this module). That loads `rusthome.toml` when present and merges `physical_projection_mode`, `io_timeout_logical_delta`, `[run_limits]`, with `--rules-preset` / `--io-anchored` overrides matching the CLI.

## Web dashboard (lab)

`rusthome serve` (or the `rusthome-web` binary) replays the same `events.jsonl` as `rusthome state` and serves a minimal HTML page plus JSON APIs (`/api/state`, `/api/journal`, `/api/command` when the broker is present). Use the **same `--data-dir`** as other subcommands. See [implementation.md — rusthome-web](implementation.md#rusthome-web-read-only-ui).

There is **no authentication**. Keep the default **`127.0.0.1`** bind for local use; if you listen on all interfaces or a LAN IP, put **TLS + access control** (reverse proxy) in front — the HTML pages show a warning when the bind is not loopback-only. Step-by-step: [web-proxy.md](web-proxy.md).

## Truth and coarse state

Observed on/off does not encode *why* the lamp is off (burnt bulb, breaker, command). See [reconciliation.md](reconciliation.md) for provenance and V0 limits.
