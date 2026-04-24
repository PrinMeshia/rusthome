# Current implementation (repository state)

This document describes **what is in code** today. Index of `docs/`: [README.md](README.md). The long plan may live in or out of repo: [plan.md](../plan.md) (often defers here for ground truth).

## Architecture scope (cleanup decisions)

- **CLI default build** ([`crates/cli`](../crates/cli/Cargo.toml)): `default` features are **empty** — no `rusthome-web` or `rumqttd` — so `cargo build -p rusthome-cli` only links the engine-facing commands. The `serve` subcommand (Axum + optional embedded broker) is enabled with `--features serve`.
- **Determinism vs MQTT** (`rusthome serve` / [`integrations::mqtt`](../crates/app/src/integrations/mqtt.rs)): the MQTT path uses `wall_millis` + `next_ts` to assign **logical** timestamps; that sequence is **not** replay-faithful the same way as explicit `emit --timestamp` — treat lab ingest as a separate contract from strict replay tests.
- **Core domain** (`rusthome-core`): all persisted `Event` variants **stay** in the build so on-disk JSONL (schemas 2–5) always round-trips. The `event` module is **split into subfiles** (lights/usage vs sensor/IO/audit groupings) for structure only — not for compile-time removal of variants. The **journal line** envelope is **`rusthome-journal`** (`JournalEntry`, `schema_version` range).
- **Orchestration errors** ([`RunError`](../crates/app/src/run_error.rs)) and **§15 trace lines** ([`RuleEvaluationRecord`](../crates/app/src/rule_trace.rs)) live in **`rusthome-app`** with the FIFO engine; the core exposes [`ApplyError`](../crates/core/src/error.rs) for pure reducer validation; line schema validation is [`JournalSchemaError`](../crates/journal/src/error.rs) in `rusthome-journal`.
- **Host runtime policy for rules**: [`HostRuntimeConfig`](../crates/core/src/host_runtime_config.rs) (trait) + [`DefaultHostConfig`](../crates/core/src/host_runtime_config.rs) in `rusthome-core`; concrete TOML snapshot [`ConfigSnapshot`](../crates/app/src/config_snapshot.rs) in **`rusthome-app`**. **Deterministic command ids** for rule emissions: [`deterministic_command_id`](../crates/rules/src/command_id.rs) in **`rusthome-rules`** (not `rusthome-core`).

## Determinism contract

What is **intended to be** reproducible and comparable across runs:

- **Double replay** / **`rusthome replay`**: same `events.jsonl` → same `State` (see tests).
- **CLI** subcommands that append with **explicit** `--timestamp` (and deterministic IDs where documented): same inputs → same journal bytes **within** a single machine clock epoch for IDs if any random `Uuid` is used — use `--command-id` / `--causal-chain-id` for fixed journals.

What is **not** offered as a golden, byte-stable contract:

- Ingest over MQTT (embedded or external broker): logical time is **derived** from `wall_millis` and monotonic `next_ts`, so two runs are not expected to match a hand-built `emit` journal.

Rule of thumb: **golden tests** use CLI or direct library ingest with explicit logical times; **lab** uses MQTT/`serve` for integration only.

Link back: [README — Determinism](../README.md#determinism).

## Plan conformance (checklist)

Legend: **OK** = reasonable V0 match to plan · **Partial** = present but incomplete or TBD · **Missing** = not in repo (or only in long plan).


| Ref.             | Topic                                                                   | Status   | Notes                                                                                                             |
| ---------------- | ----------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------- |
| §3.2–3.4         | Logical time, `(timestamp, sequence)` order, timestamp gate on append    | OK       | Infra rejects regressive `timestamp`.                                                                              |
| §3.5–3.6         | 3 families + Observed/Derived provenance                                | OK       | Enums + fact fields.                                                                                             |
| §4               | `apply_event` facts-only, `Result`, pre-append validation               | OK       | `validate_fact_for_append` + pipeline.                                                                            |
| §4.4             | `State` / `StateView` encapsulation                                     | OK       | `pub(crate)` fields; reducer alone mutates.                                                                       |
| §5               | Infra sort, app no re-sort                                              | OK       | `load_and_sort`.                                                                                                  |
| §6.2–6.3         | FIFO, synchronous derived append, timestamp / causality inheritance     | OK       | `drain_fifo` + append with metadata.                                                                           |
| §6.6.1 / .3 / .4 | Event caps, wall-clock budget, bounded queue                              | OK       | `RunLimits` + `Instant` outside domain logic.                                                                      |
| §6.6.2           | Per-root anti-explosion                                                 | OK       | Causality tree from **first** queue event at drain start; counter on appended descendants.       |
| §6.9–6.10        | Registry consumes/produces, Option B API                                | OK       | Trait + `Registry::validate_boot`.                                                                                |
| §6.12            | `BTreeMap` / no fragile `HashMap` in `State`                            | OK       |                                                                                                                   |
| §6.12.1          | Rule purity guards                                                      | OK       | `clippy.toml` + boot tests; [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) (fmt, clippy rules `-D warnings`, clippy workspace, `cargo build -p rusthome-app --examples`, tests). |
| §6.13            | Static graph, cycles → boot fail                                        | OK       | DFS on kinds.                                                                                                    |
| §6.14–6.15       | Namespaces, fan-in ≤ 3                                                  | OK       | Constant + validation.                                                                                           |
| §6.17            | Family transition matrix                                                | OK       | V0 default + `Registry::family_transition_whitelist` (non-redundant entries validated at boot).                   |
| §6.16            | IO cycle (EPIC 2)                                                       | OK       | `Dispatched { logical_deadline }` → `acked` \| `failed` \| `timeout`; tracking by `room`/`command_id`; R3 simulation: Dispatched + Acked; shadow validate pipeline — [io-lifecycle.md](io-lifecycle.md). |
| §6.18            | Anti-oscillation / shared-axis tests                                    | Partial  | Discipline [onboarding-rules.md](onboarding-rules.md); `shared_axis_invariant`; `determinism_proptest` (motion sequences + **same §6.6 `RunLimits`** → identical journal / `RunError`). **Multi-rule** oscillation off acyclic graph still optional.                    |
| §7               | Single-node, sequential                                                 | OK       | Code model + README.                                                                                             |
| §7.1             | Throughput, p95 SLO, micro-bench                                      | Partial  | `rusthome bench` + [`scripts/bench-p95.sh`](../scripts/bench-p95.sh) (multiple runs); p95 under real load to calibrate manually. |
| §8.0             | JSON Lines UTF-8                                                        | OK       |                                                                                                                   |
| §8.1             | Synchronous append, single writer                                       | OK       | Global CLI `--journal-fsync` + `Journal::set_fsync_after_append`.                                          |
| §8.2             | `schema_version`                                                        | OK       | **5** current append; load accepts **2..=5** via `journal_schema_supported` / `JournalEntry::validate_supported_schema` ([rules-changelog.md](rules-changelog.md)); `ErrorOccurred` (EPIC 4); humidity facts (schema 5); see [errors.md](errors.md). |
| §8.3             | Canonical JSON (sorted keys, no floats)                                 | OK       | `to_canonical_line` (recursive sort); top-level key test + infra round-trip.                                      |
| §8.4             | Snapshot + `state_hash`                                                 | OK       | `snapshot` + `emit --write-snapshot` (digest via `--snapshot-rules-digest` / `--rules-digest`, default `rules-v0` \| `rules-home` \| `rules-minimal` per preset).                    |
| §8.5             | Corruption, fail fast, repair                                           | OK       | Strict parse, `repair_journal` + CLI `repair`.                                                                    |
| §9 / CLI         | No implicit `now`                                                       | OK       | `emit --timestamp` required.                                                                                   |
| §14.1            | Non-idempotence documented                                              | OK       | Comments on `replay_state` / ingest in `rusthome-app`.                                                       |
| §14.3            | `command_id` required + append dedup                                    | OK       | `CommandEvent` requires `Uuid`; `rusthome_rules::deterministic_command_id` (v5); `Journal::append` → `DuplicateCommandSkipped` if id already seen (disk index at `open`). |
| §14.5            | `physical_projection_mode`                                              | OK       | Test `io_anchored_rejects_derived_light_from_rule` + CLI `--io-anchored`.                                          |
| §14.6            | Dead letter / quarantine ideas                                          | OK       | [reconciliation.md](reconciliation.md) + [errors.md](errors.md).                                                  |
| §14.7            | Journal ↔ world reconciliation                                          | OK       | [reconciliation.md](reconciliation.md) — Observed / Derived invariant + `StateCorrectedFromObservation` + `append_observed_light_fact`. |
| §14.8            | Fatal vs recoverable errors                                             | OK       | [errors.md](errors.md).                                                                                             |
| §15              | `rule_id`, `parent_`*, `causal_chain_id`                                | OK       | Persisted on relevant lines.                                                                             |
| §15              | Trace `matched` / `not matched`                                         | OK       | `rusthome_app::RuleEvaluationRecord` + `emit --trace-file` + pipeline param; one line per rule and processed event.   |
| §15              | `correlation_id` / `trace_id` schema reserved                           | OK       | Optional fields on `JournalEntry` (propagated to derived); root often `null`.                               |
| §19–§20          | `rules_digest` / rules version                                          | OK       | Snapshot + [rules-changelog.md](rules-changelog.md).                                                                |
| §22–§23          | Auto graph doc, onboarding guide                                        | OK       | `rules-doc` (Mermaid) + [onboarding-rules.md](onboarding-rules.md).                                                   |
| §24              | “Why” surface, CLI `explain`                                              | OK       | `explain --causal <uuid>` + `causal_chain_id` filter; file trace for “matched”.                             |
| §16              | Non-regression scenario                                                 | OK       | `crates/app/tests/scenario_16.rs`.                                                                                |


**Summary**: major V0 gaps are closed. **§6.18**: determinism proptest (including cascade caps) + shared-axis invariants; multi-rule oscillation (graph) out of scope. **§7.1**: p95 under real load to measure in lab.

## Cargo workspace


| Crate              | Role                                                                                                                                                            |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **rusthome-core**  | Event types (3 families), `State`, `apply_event` / `validate_fact_for_append`, `Rule` / `StateView` traits, `ApplyError` |
| **rusthome-journal** | Persisted line shape: `JournalEntry` metadata + flattened `Event`, `SCHEMA_VERSION` / `MIN_SUPPORTED_JOURNAL_SCHEMA`, `JournalSchemaError` on unsupported schema |
| **rusthome-rules** | Demo rules (R1–R5 + R7 `TurnOffLight`), `deterministic_command_id`, presets `v0` / `home` (no R2) / `minimal`, registry, boot validation (cycles §6.13, fan-in §6.15, family transitions §6.17, `produces` consistency) |
| **rusthome-app**   | FIFO pipeline, `RunError`, `RuleEvaluationRecord` (§15), `ConfigSnapshot` (TOML + `HostRuntimeConfig`), `rusthome_file` (`rusthome.toml`, presets, `RunLimits`); integrations (`integrations::mqtt`, …) |
| **rusthome-infra** | **Canonical** JSON Lines journal §8.3, `JournalAppend` / `JournalAppendOutcome` (command dedup §14.3), sort, timestamp gate, snapshot + `state_hash`, `repair_journal`, optional `fsync` |
| **rusthome-cli**   | `rusthome` binary (clap). Optional Cargo feature `serve` pulls in `rusthome-web` + `rumqttd` for the `serve` subcommand. |
| **rusthome-web**   | Library + `rusthome-web` binary; also **`rusthome serve`** when CLI is built with `--features serve` — read-only Axum: dashboard `/`, system `/system`, JSON APIs incl. `/api/system` (lab; default bind `127.0.0.1:8080`) |


**No wall clock** in domain logic: CLI requires explicit `--timestamp` on `emit`; runner uses `Instant` only for run time budget (§6.6.3), not event ordering.

## Event model (persisted)

Each journal line is a `JournalEntry`: `schema_version`, `timestamp` (logical time), `sequence` (global, assigned by infra on append), `causal_chain_id`, `parent_*`, optional `rule_id` on derived lines, `event_id`, **`correlation_id`**, **`trace_id`** (optional §15), plus flattened serde body `Event`:

- **Fact** — only family reduced by `apply_event`; `Observed` / `Derived` provenance on facts.
- **Command** — intent; domain fields + **`command_id: Uuid` required** (EPIC 3); determinism via `rusthome_rules::deterministic_command_id`.
- **Observation** — inbound signal (e.g. `MotionDetected`).
- **Error** — `ErrorOccurred` (EPIC 4): logged on drain failures; **not** applied to `State` on replay.

Read/replay order is **only** `(timestamp, sequence)`.

## Projected state (`State`)

- `lights`: `BTreeMap` (deterministic iteration).
- `last_log`: demo usage (`UsageLogged` facts).
- Mutation **only** via `apply_event` on facts; internal fields `pub(crate)`.

## On-disk data (default CLI)

Under `--data-dir` (default: `data/`):


| File            | Role                                     |
| --------------- | ---------------------------------------- |
| `events.jsonl`  | Append-only journal                      |
| `snapshot.json` | Optional snapshot (`snapshot` command)   |
| `rusthome.toml` | Optional: preset, physical mode, IO delta, `[run_limits]`; validated on load; e.g. [configs/rusthome.example.toml](../configs/rusthome.example.toml) |


## CLI `rusthome`


| Command | Description |
| -------- | ----------- |
| `emit --timestamp … [--room …] [--io-anchored] [--trace-file PATH] [--write-snapshot] [--snapshot-rules-digest …]` | Motion observation + cascade; §15 trace; runtime config from `rusthome.toml`; snapshot after run if requested |
| `turn-off-light --timestamp … [--room …] [--command-id UUID] [--causal-chain-id UUID] [--io-anchored] [--trace-file PATH] [--write-snapshot]` | `TurnOffLight`; R7 → `LightOff` + `CommandIo` (Simulation); IoAnchored rejects Derived actuator like `emit`; §15 trace optional |
| `state` | Replay → JSON `State` |
| `replay` | Double replay |
| `snapshot [--rules-digest …]` | Writes `snapshot.json` (default digest = preset) |
| `repair [--backup-suffix …]` | §8.5 |
| `explain --causal <uuid>` | Journal entries for one cascade |
| `rules-doc` | Mermaid consumes→produces graph |
| `bench --count N` | Rough ingest measurement (temp journal) |
| `serve` | Lab: embedded MQTT broker (optional) + web dashboard; **requires** building `rusthome-cli` with `--features serve` |
| `observed-light --timestamp … --room … --state on|off [--io-anchored] [--write-snapshot]` | **Observed** light fact + correction if **Derived** projection diverges |

Global: `--data-dir` (env `RUSTHOME_DATA_DIR`), `--rules-preset` (env `RUSTHOME_RULES_PRESET`, then file), `--journal-fsync`. `rusthome.toml`: strict parse + validation; optional `[run_limits]` (§6.6) for cascade caps.

### `rusthome-web` (read-only UI)

| Route | Description |
| ----- | ------------- |
| `GET /` | HTML dashboard: lights, journal tail, usage log (replay + live refresh) |
| `GET /system` | HTML « system »: rusthome paths, host/OS, CPU, RAM, swap, load, sensors, disk, Bluetooth adapters (read-only) |
| `GET /api/state` | JSON projection (`State`) |
| `GET /api/journal?limit=N` | Last N lines (default 40, max 500): `sequence`, `timestamp`, `kind` |
| `GET /api/system` | JSON host/resources snapshot (same fields as system page) |
| `GET /api/bluetooth` | JSON Bluetooth: adapters (sysfs + `bluetoothctl show`) and `devices[]` (`bluetoothctl devices`, optional Paired/Connected flags), read-only |
| `GET /api/health` | `{"ok":true}` |

Run: **`rusthome serve`** (uses global `--data-dir`) after `cargo run -p rusthome-cli --features serve --`, or `cargo run -p rusthome-web -- --data-dir data`. **`serve`**: `--bind 127.0.0.1:8080` (default). Env `RUSTHOME_DATA_DIR` supported. **Not hardened** — local / lab only.

**Security:** default bind is loopback. If you use `--bind 0.0.0.0:…` or another non-loopback address, the HTML UI shows a warning banner: APIs and system/Bluetooth views are **unauthenticated**. For LAN access, prefer a **reverse proxy** (TLS, auth, firewall) rather than exposing the Axum port directly. See [web-proxy.md](web-proxy.md) and `configs/Caddyfile.rusthome.example`, `configs/nginx-rusthome.conf.example`.

## Rule registry (boot)

`Registry::validate_boot()` checks among other things:

- no **cycle** on consumed-kind → produced-kind graph;
- **§6.15**: at most 3 consumed types per rule;
- **§6.17**: allowed transitions between Observation / Command / Fact families (+ boot whitelist without redundancy);
- sample contract: anything `eval` may emit must appear in `produces`.

The **rules** crate ships `clippy.toml` (discourage system time types) for §6.12.1.

## Notable tests

- **rusthome-core**: `apply_event`, `validate_fact_for_append`.
- **rusthome-journal**: `JournalEntry` / supported `schema_version` range (line contract, distinct from `Event` body in `rusthome-core`).
- **rusthome-rules**: V0 registry, `command_id` helpers, reject emission outside `produces`, reject Fact→Fact outside policy.
- **rusthome-app**: `scenario_16.rs`, `policy_and_trace.rs`, `truth_convergence.rs`, `io_lifecycle.rs`, `command_dedup.rs` (EPIC 3), `error_occurred_replay.rs` (EPIC 4), `determinism_proptest.rs`, `shared_axis_invariant.rs`, `preset_minimal.rs` / `preset_home.rs`; unit tests for `RunLimits` in `pipeline.rs`.
- **rusthome-infra**: `canon` round-trip, journal load vs schema.

```bash
cargo test --workspace
cargo run -p rusthome-cli -- --help
# Local CI (GitHub Actions equivalent): cargo fmt --check, clippy rules -D warnings, clippy workspace, tests
```

## Not covered or partial (see table above)

Property tests §6.18 **multi-rule** with oscillating graph (off acyclic registry), **p95** under real load §7.1.

## Related documents

Index: **[docs/README.md](README.md)**. Also: [perf-assumptions.md](perf-assumptions.md); [errors.md](errors.md), [reconciliation.md](reconciliation.md), [integration.md](integration.md); [onboarding-rules.md](onboarding-rules.md), [user-rules.md](user-rules.md), [rules-changelog.md](rules-changelog.md); [io-lifecycle.md](io-lifecycle.md); [roadmap-2-semaines.md](roadmap-2-semaines.md).
