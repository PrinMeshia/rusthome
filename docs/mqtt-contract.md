# MQTT ingest contract (version 2)

**Contract version:** `2` (bump when topic shapes or payload rules change; describe migration in [integration.md](integration.md) and [rules-changelog.md](rules-changelog.md) if rules depend on new fields).

**Source of truth:** `[crates/app/src/mqtt_ingest.rs](../crates/app/src/mqtt_ingest.rs)` (`observation_from_mqtt`, `command_from_mqtt`, `dispatch_mqtt_publish`).

**Timestamps:** Logical journal time uses `next_ts(last_ts, candidate)` where `candidate` is optional JSON `"ts"` (milliseconds) or **wall-clock** via `wall_millis()` when absent. The **domain model** stays free of mandatory wall-clock; MQTT is an edge adapter (plan §3).

## Topic → event mapping


| Topic pattern                                     | Event                                              | Notes                                        |
| ------------------------------------------------- | -------------------------------------------------- | -------------------------------------------- |
| `sensors/motion/{entity}`                         | `MotionDetected { room }`                          | `{entity}` used if payload does not override |
| `sensors/temperature/{entity}`                    | `TemperatureReading { sensor_id, millidegrees_c }` | See payloads below                           |
| `sensors/contact/{entity}`                        | `ContactChanged { sensor_id, open }`               |                                              |
| `sensors/humidity/{entity}`                       | `HumidityReading { sensor_id, permille_rh }`       | 0–1000 permille RH (see payloads)            |
| `commands/light/{room}/on`                        | `TurnOnLight { room, command_id }`                 | `command_id` generated at ingest             |
| `commands/light/{room}/off`                       | `TurnOffLight { room, command_id }`                |                                              |
| Other `commands/...`                              | *skipped*                                          | `Ok(None)` — no journal line                 |
| Unknown `sensors/...` category                    | *skipped*                                          | e.g. `sensors/pressure/...` → `None`         |
| `commands/light/{room}` (missing `/on` or `/off`) | **error**                                          | Malformed — surfaces as parse error          |


Wildcards on subscribe (e.g. `sensors/#`, `commands/#`) are supported; classification uses the **concrete** topic string.

## Payloads — observations

**Motion**

- JSON: `{"room":"<name>"}` (preferred if topic entity is a placeholder).
- Plain UTF-8 short string (no `{`): used as room name.
- Empty: entity taken from last segment of topic (`sensors/motion/hall` → `hall`).

**Temperature** (`millidegrees_c` in core)

- JSON: `millidegrees_c`, or `celsius` (float → ×1000), or `value` as integer millidegrees.
- Plain integer string: parsed as millidegrees.

**Contact**

- JSON: `open: bool`, or Zigbee2MQTT-style `contact: true` meaning **closed** (inverted to `open = false`).
- Plain: `open` / `closed` / `true` / `false` / `0` / `1` (see source for exact tokens).

**Humidity** (`permille_rh` in core: 0 = 0%, 1000 = 100%)

- JSON: `permille_rh` (integer), or `relative_humidity` / `humidity` as **percent** 0–100 (float → permille).
- Plain number string: interpreted as **percent** 0–100 (same as `humidity` float), converted to permille.

Optional: `"ts": <i64>` milliseconds — passed as timestamp **candidate** (still monotonic with `next_ts`).

## Payloads — commands

Ignored for classification; empty payload is fine. (UUIDs in commands are generated at ingest, not from MQTT.)

## External stacks (Zigbee2MQTT, Tasmota, Home Assistant)

Rusthome expects its **own** topic prefix (`sensors/...`, `commands/...`). Typical bridges:

1. **Topic rewrite** in the MQTT bridge (e.g. map `zigbee2mqtt/device` → `sensors/motion/kitchen` with a JSON template that sets `room` / `sensor_id` and temperature fields).
2. **Second subscriber** (fork of `[mqtt_motion_ingest](../crates/app/examples/mqtt_motion_ingest.rs)`) that maps foreign topics in Rust before calling `dispatch_mqtt_publish`.
3. **Embedded broker** (`rusthome serve`): publish directly to the contract topics from HA / Node-RED.

Do **not** rely on foreign timestamps for ordering across devices unless you also enforce monotonicity upstream; the journal only sees merged `next_ts` behaviour.

## Version history


| Version | Summary                                                                |
| ------- | ---------------------------------------------------------------------- |
| 1       | Initial documented contract (matches `mqtt_ingest` as of Phase 2 plan) |
| 2       | Adds `sensors/humidity/{entity}` → `HumidityReading` (permille RH 0–1000) |
