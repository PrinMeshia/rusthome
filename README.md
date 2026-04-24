# rusthome — deterministic home automation (V0)

**rusthome** is a Rust **event-sourced** engine: observations and commands are appended as lines in a **JSON Lines** journal; **state** is the projection obtained by replaying **facts** in order; **rules** react to each event in a FIFO pipeline and may append more lines. The focus is **reproducibility** (same journal → same state) and **clear crate boundaries** — not a finished home product.

V0 validates the architecture; many product features are intentionally out of scope (see [Scope](#scope-and-out-of-scope)).

---

## Documentation (read this first)

| If you want… | Open |
| ------------ | ---- |
| **What the code does today** (crates, CLI, on-disk files, tests) | [`docs/implementation.md`](docs/implementation.md) |
| **Index of all `docs/`** | [`docs/README.md`](docs/README.md) |
| **Adapters, MQTT lab, `serve`, library examples** | [`docs/integration.md`](docs/integration.md) |
| **Long-form design reference** | [`plan.md`](plan.md) |

---

## Quick commands

```bash
# Full test suite (CI-style)
cargo test --workspace

# CLI help
cargo run -p rusthome-cli -- --help

# Optional lab stack: embedded broker + web UI (not in default build)
cargo run -p rusthome-cli --features serve -- serve --bind 127.0.0.1:8080
```

Default data directory for the CLI is `./data` (override with `--data-dir` or `RUSTHOME_DATA_DIR`). Example config: [`configs/rusthome.example.toml`](configs/rusthome.example.toml).

---

## How the workspace is split

Each crate has one main job. **Dependency direction** (simplified): `journal` → `core`; `rules` → `core`; `infra` → `core` + `journal`; `app` → `core` + `journal` + `rules` + `infra`; `cli` → `app` + …

| Layer | Crate | What it owns |
| ----- | ----- | ------------ |
| **Domain** | `rusthome-core` | `Event` variants, `State`, `apply_event` / `validate_fact_for_append`, `Rule` / `StateView`, `HostRuntimeConfig` trait. **No** file/network I/O, **no** TOML, **no** `JournalEntry` line type. |
| **Journal line** | `rusthome-journal` | `JournalEntry` (metadata + flattened `Event`), `schema_version`, `SCHEMA_VERSION` / supported range, `JournalSchemaError`. |
| **Rules** | `rusthome-rules` | Bundled rules (R1…), `Registry`, `deterministic_command_id`, boot validation. |
| **Orchestration** | `rusthome-app` | FIFO `drain_fifo`, `ConfigSnapshot` (TOML), `RunError`, optional rule trace, MQTT ingest when used. |
| **Persistence** | `rusthome-infra` | JSONL read/write, sort, `sequence`, snapshot, repair, append dedup on `command_id`. |
| **CLI** | `rusthome-cli` | The `rusthome` binary. |
| **Web (lab)** | `rusthome-web` + `serve` | Read-only dashboard + JSON APIs when built with `--features serve` — **no auth**; bind `127.0.0.1` unless you know what you are doing. |

The **rule engine** (loop + append) and **`RunError`** / **`RuleEvaluationRecord`** live in **app**; **rule implementations** live in **rules**.

---

## Repository layout

```
rusthome/
  Cargo.toml              # workspace
  crates/
    core/                 # rusthome-core
    journal/              # rusthome-journal
    rules/                # rusthome-rules
    app/                  # rusthome-app
    infra/                # rusthome-infra
    cli/                  # rusthome-cli (rusthome binary)
    web/                  # rusthome-web (optional, feature-gated in CLI)
  configs/                # e.g. rusthome.example.toml
  data/                   # default CLI: journal + snapshot
  docs/                   # see docs/README.md
  plan.md                 # long design slot (may defer to docs/)
```

---

## Core ideas

### Determinism

For paths that use **fixed logical time** (e.g. CLI `emit` with explicit `--timestamp`, replay of a **closed** journal), **same inputs → same journal / same state** — see the [determinism section](docs/implementation.md#determinism-contract) in `docs/implementation.md`.

**Lab exception:** MQTT and `serve` use **wall-assisted** logical timestamps (`wall_millis` / `next_ts`). Journals produced that way are **not** bit-identical to a hand-built CLI journal — by design: lab = live, not a golden file.

**Domain code** (`rusthome-core` reducer, `State`, `Rule::eval`) must **not** use the wall clock for business logic.

### Event-driven model

The system is driven by **persisted** lines. Three **families** are implemented: **Fact**, **Command**, **Observation** (see `docs/implementation.md`).

### Time on the journal

Each line has a **logical** `timestamp` (`i64`). **Global order** is **`(timestamp, sequence)`** — `sequence` is a monotonic counter assigned on append. Live append **rejects** a timestamp **below** the last committed one (consistency over completeness; plan §3.4). **Do not** use `SystemTime` / `Utc::now()` inside domain rules for ordering decisions.

### Derived vs persisted

Values derived from the journal context do not replace **persisted facts** as the source of durable state.

---

## Journal line shape (summary)

Each line combines metadata and a **flattened** `Event` body (full detail: `docs/implementation.md`).

- `timestamp`, `sequence` — **total order**
- `schema_version` — on-disk era (current append and supported load range: see `rusthome-journal` and `docs/schema-migration.md`)
- optional: `event_id`, `causal_chain_id`, `parent_*`, `rule_id`, `correlation_id`, `trace_id` (observability §15)

On live append, Infra enforces the timestamp gate and assigns `sequence`.

---

## State and rules (summary)

- **State** is the **projection** from **facts** in journal order. **Commands** and **observations** do not mutate projection directly; they drive rules, which may append **facts**.

- **Rules** declare `consumes` / `produces` / `rule_id` / `priority` / `namespaces`. After a dequeued **fact**, the reducer runs **`apply_event`**; then rules run. Emissions are ordered by **priority (desc)**, then **`rule_id`**, then order inside the rule; each synthetic is **appended** then enqueued (FIFO). Caps: `RunLimits` (event limits, wall budget, queue size — plan §6.6).

- **Conflict handling:** numeric priorities, then `rule_id` lexicographic, then stable per-rule order — **no randomness** in the engine.

- **Lights** in projected state use **`BTreeMap`** (not `HashMap`) for stable iteration where it matters.

---

## CLI (V0)

| Command | What it does (short) |
| ------- | -------------------- |
| `emit` | Append motion (and cascade); requires `--timestamp`; optional `rusthome.toml`, `--trace-file` |
| `turn-off-light` | `TurnOffLight` + R7; optional `--command-id` / `--causal-chain-id` |
| `observed-light` | Observed light fact + reconciliation (§14.7) |
| `state` / `replay` | Print or verify projection; double-replay tests |
| `snapshot` / `repair` | Snapshot + `state_hash`; repair corrupted JSONL (§8.5) |
| `explain` / `rules-doc` | Causality and Mermaid rule graph |
| `bench` | Micro-benchmark on a temp journal |

**Web (lab):** `rusthome serve` after `cargo build -p rusthome-cli --features serve` — e.g. [http://127.0.0.1:8080](http://127.0.0.1:8080). Read-only; use a reverse proxy if exposed beyond loopback.

Full flags, env vars (`RUSTHOME_DATA_DIR`, `RUSTHOME_RULES_PRESET`), presets, digests: **[`docs/implementation.md` — CLI](docs/implementation.md#cli-rusthome)** and `rusthome --help`. Library-style usage: [`docs/integration.md`](docs/integration.md).

---

## Replay

Load the journal in **`(timestamp, sequence)`** order, apply **facts only** to rebuild `State`. The usual `state` / `replay` flow does **not** append to the canonical journal.

---

## Scope and out-of-scope

**In scope (implemented in tree):** canonical JSON Lines journal, snapshot, repair, optional `fsync`, three families + `CommandIo`, facts-only reducer, FIFO pipeline, rule trace §15, IoAnchored guard §14.5, registry boot checks, CLI listed above, integration tests and proptests.

**Out of scope or partial:** full UI, WASM plugins, multi-node cluster, p95 SLO under real load (see `docs/perf-assumptions.md`), **real device drivers** (network/GPIO) — the **journal model** (`CommandIo`, `command_id`, dedup) is ready; wiring physical devices is up to integrators. See also [`docs/roadmap-2-semaines.md`](docs/roadmap-2-semaines.md).

**Security (future):** sandboxed plugins, auth on the web UI, zero-trust — not V0.

---

## Success criteria (V0)

- Reproducible ordering and **pure** rule evaluation (no wall clock in domain)
- **Identical replay** from the same journal
- **Testable** workspace: `cargo test --workspace`
- **Explainable** layout: crates and docs map to behavior

---

## Risks to watch

Over-engineering, scope creep, unnecessary abstraction — the intended direction is **simple, deterministic, explainable**; everything else can come later.
