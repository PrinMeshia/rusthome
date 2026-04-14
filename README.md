# Project: Deterministic Home Automation (V0)

**Documentation**: index **[docs/README.md](docs/README.md)** — start with **[docs/implementation.md](docs/implementation.md)** (code state). Edge adapters: **[docs/integration.md](docs/integration.md)**. Long-form reference: [plan.md](plan.md).

## 1. Project goal

Build a minimal, deterministic, extensible home automation engine focused on:

- an event-driven model
- reproducible execution (replay)
- strict separation of concerns
- a solid base for future extensibility (plugins, WASM)

V0 is not meant to be complete; it validates the foundations.

---

## 2. Core principles

### 2.1 Determinism

Same input → same output, always.

Implications:

- no wall clock in domain logic (`Utc::now()`, etc.)
- **logical** timestamps supplied explicitly (e.g. CLI `emit --timestamp`)
- replay matches initial behaviour (same journal, same binary, same config)

### 2.2 Event-driven

The system is built on **persisted events** in a journal. Three **families** (implemented): **Fact**, **Command**, **Observation** — see [docs/implementation.md](docs/implementation.md).

### 2.3 Strict separation of concerns


| Layer        | Crate            | Role                                                    |
| ------------ | ---------------- | ------------------------------------------------------- |
| Pure domain  | `rusthome-core`  | Types, reducer (`apply_event` **facts only**), contracts |
| Rules        | `rusthome-rules` | `bundle` (`v0` R1–R5, `home` without R2, `minimal` R1+R3), `Registry::from_rules`, boot validation |
| Orchestration | `rusthome-app`   | FIFO, synchronous append of derived events, run caps     |
| Persistence  | `rusthome-infra` | JSONL, `sequence`, snapshot, repair                     |
| Interface    | `rusthome-cli`   | Command line                                            |
| Web (dev)    | `rusthome serve` / `rusthome-web` | Dashboard `/`, system `/system`, JSON (`/api/state`, `/api/system`, …) — **no auth**; default bind `127.0.0.1`; use a reverse proxy if you listen on the LAN  |


### 2.4 Explicit time

The **timestamp** on a journal line is **logical** ordering time (not wall-clock time or real latency between cascade steps). See plan §3.7.

Forbidden in domain logic: deriving decisions from `SystemTime` / `Utc::now()`.

### 2.5 Derived data

Values computed from event context (logical timestamp, projected state) do not replace persisting **facts** for durable state.

---

## 3. Repository layout (Rust)

```
rusthome/
  Cargo.toml                 # workspace
  crates/
    core/                    # rusthome-core
    rules/                   # rusthome-rules
    app/                     # rusthome-app
    infra/                   # rusthome-infra
    cli/                     # rusthome-cli (rusthome binary)
  configs/                   # e.g. rusthome.example.toml
  data/                      # default CLI: journal + snapshot
  docs/                      # index README.md, implementation.md, …
  plan.md                    # long plan slot (often points to docs/)
```

Useful commands:

```bash
cargo test --workspace
cargo run -p rusthome-cli -- --help
```

---

## 4. Core

Responsibilities:

- event enums and `JournalEntry`
- `State` (projection), `StateView` for rules
- `apply_event` / `validate_fact_for_append` (**Result**, fail-fast domain)
- `Rule` / `RuleContext` traits

Constraints:

- **no file/network IO**
- limited dependencies (serde, uuid, thiserror) — no `chrono` for time logic

The **rule engine** (FIFO loop, append) lives in **app**; **rule implementations** live in **rules**.

---

## 5. Event model (journal)

Each persisted line includes:

- **`timestamp`**: `i64` (logical time)
- **`sequence`**: `u64` — **global** monotonic counter assigned by infra on each successful append (tie-break when timestamps tie)
- **`event_id`**: optional (traceability), **not** used for global order
- **`causal_chain_id`**, **`parent_sequence`** / **`parent_event_id`**, **`rule_id`** (derived): observability §15
- **body**: Fact / Command / Observation (serde tag)

**Total order**: `(timestamp, sequence)` only.

On **live** append, a timestamp strictly below the last committed one is **rejected** (consistency > completeness — plan §3.4).

---

## 6. State

- Represents the **projection** replayable from journal **facts**.
- Internal fields exposed read-only via **`StateView`** for rules; mutation **only** through `apply_event` on facts.
- Determinism-sensitive collections: **`BTreeMap`** for lights (no `HashMap` for projected state).

---

## 7. Rule engine (implemented behaviour)

- A rule declares **`consumes`**, **`produces`**, **`rule_id`**, **`priority`**, **`namespaces`**.
- For each dequeued event: if **Fact**, **`apply_event`** first, then rule evaluation (rules can consume the newly true fact — e.g. plan §16).
- Actions are sorted by **`(priority desc, rule_id, action_ordinal)`**, then executed sequentially; each emitted event is **appended** immediately then enqueued (FIFO).

Configurable caps (`RunLimits`): processed events, generated events, wall-clock budget, queue size — plan §6.6.

---

## 8. Ordering

1. **`timestamp`** (logical time)
2. **`sequence`** (global, persisted)

Do not rely on network arrival order without a layer that respects §3.4.

---

## 9. Conflict handling / tie-breaks

- Numeric priorities on actions, then **`rule_id`** lexicographic, then stable action order inside the rule.
- No random behaviour.

---

## 10. CLI (V0 implemented)

| Command | Role (summary) |
| -------- | ---------------- |
| `emit` | Motion + cascade; `--timestamp`; `--trace-file`; `rusthome.toml` |
| `turn-off-light` | `TurnOffLight` + R7; `--trace-file`; optional `--command-id` / `--causal-chain-id` |
| `observed-light` | Observed light fact + reconciliation §14.7 |
| `state` / `replay` | JSON projection; double replay |
| `snapshot` / `repair` | Snapshot + `state_hash`; journal repair §8.5 |
| `explain` / `rules-doc` | Cascade by `causal_chain_id`; Mermaid consumes→produces |
| `bench` | Micro-bench ingest (tmp journal) |

**Web (lab):** `rusthome serve` (same as `rusthome-web --data-dir …`) → [http://127.0.0.1:8080](http://127.0.0.1:8080) — read-only projection + JSON. Global `--data-dir` / `RUSTHOME_DATA_DIR`; `--bind` on `serve`. No auth; keep local bind.

Flags, env (`RUSTHOME_DATA_DIR`, `RUSTHOME_RULES_PRESET`), `rusthome.toml`, digests: **[docs/implementation.md](docs/implementation.md#cli-rusthome)** · `rusthome --help` · [configs/rusthome.example.toml](configs/rusthome.example.toml). Library templates: [docs/integration.md](docs/integration.md) (`append_observed`, `ingest_turn_off` examples).

---

## 11. Replay

- Read sorted journal, apply **facts only** to rebuild `State` (commands/observations do not mutate projection directly).
- Replay / `state` mode: **no append** to the canonical journal in the current CLI flow.

---

## 12. Security (future)

Planned but out of V0 scope:

- WASM plugins, sandbox, permissions, zero trust

---

## 13. V0 scope (current code)

**Included (implemented)**:

- **canonical** JSON Lines journal (sorted keys §8.3), snapshot, repair, optional `fsync`
- three families + `CommandIo` fact (§6.16, state no-op); facts-only reducer, pre-append validation
- FIFO pipeline + rule trace §15 + **IoAnchored** guard §14.5 + per-root cap §6.6.2
- registry + boot validation (cycles, fan-in, family transitions, `produces` consistency)
- CLI (`explain`, `rules-doc`, `bench`, …)
- §16 tests + policy / trace / canon

**Excluded or partial**:

- UI, WASM plugins, multi-node
- **Real device** integration (network, GPIO, etc.) — engine and `CommandIo` / `command_id` are in place on the journal side

---

## 14. Long plan scope

Milestones from the detailed plan (in or out of repo) guided the architecture. **Exact code state**: [docs/implementation.md](docs/implementation.md); possible next steps: [docs/roadmap-2-semaines.md](docs/roadmap-2-semaines.md).

---

## 15. Success criteria

- full determinism (ordering + rule purity in rules)
- identical replay
- clear architecture (crates)
- testable code (`cargo test --workspace`)

---

## 16. Main risks

- over-engineering
- scope creep
- unnecessary complexity

---

## 17. Guiding principle

Build simple, deterministic, explainable.

Everything else can follow.
