# Command IO lifecycle (plan §6.16, EPIC 2)

## Journal type: `FactEvent::CommandIo`

Fields: `phase`, optional `command_id`, optional **`room`** (tracking key if no id), `provenance`.

Phases (JSON):

- **`dispatched`** — object with optional `logical_deadline` (logical time §3: beyond this, a watchdog / IO layer may append `timeout`).
- **`acked`** — hardware success (deserialization alias: legacy `succeeded`).
- **`failed`** — `{ reason }`.
- **`timeout`** — no response in the logical window; **at most one** new `dispatched` allowed after (see `State.command_io_trackers`).

**Domain command** (`TurnOnLight`, …) = **issued** intent (Command family in the journal); it is not a `CommandIo` fact.

- `apply_event` does **not** change lights for `CommandIo`: trace + lifecycle tracking in `command_io_trackers`.
- **R3 (simulation)**: after `LightOn`, append **`CommandIo` Dispatched** first (deadline = trigger timestamp + `config.io_timeout_logical_delta`), then **Acked** — never `acked` without a `dispatched` line (validated at replay + **shadow state** in the pipeline for one emission batch).
- **IoAnchored**: no Derived `LightOn` from R3 (unchanged); in prod, the IO layer appends `dispatched` then `acked` / `failed` / `timeout` from the driver.

## Emitting `timeout`

Compare **logical timestamp** of new events (ingest / tick) to `logical_deadline` of open `Dispatched` rows (out of scope for the reducer alone); append a `CommandIo { phase: timeout, … }` fact with the same `room` / `command_id`. Not automated in the V0 binary: documented hook for driver integration.
