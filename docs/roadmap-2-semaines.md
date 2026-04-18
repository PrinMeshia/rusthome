# Roadmap — history and next steps

> The old “two-week” plan (canonical JSON, trace §15, IO §6.16, `errors.md`, Pi bench, `rules-doc`, etc.) is **largely done**; current detail is in [implementation.md](implementation.md).

## Next steps (short backlog)

| Priority | Topic | Notes |
| -------- | ----- | ----- |
| Tests | Property §6.18 (§6.6 caps, determinism) | `determinism_proptest` + `oscillation_proptest` (deep graph, `max_pending_events`); see `scripts/proptest-suite-p95.sh` and [testing-core.md](testing-core.md) |
| Perf | §7.1 — p95 under load on Pi | `bench`, `scripts/bench-p95.sh`, [perf-assumptions.md](perf-assumptions.md) |
| Integration | MQTT contract + golden path | [mqtt-contract.md](mqtt-contract.md), [integration.md](integration.md) « Golden path », [scenarios.md](scenarios.md) |
| Rules | `home` preset ≠ `v0` | **Done** — `arc_rules_home()` = R1+R3+R4+R5 (no R2) |
| Schema | New event types (beyond light) | Bump `SCHEMA_VERSION` only when adding/changing persisted `Event` shapes — follow [schema-migration.md](schema-migration.md) and [rules-changelog.md](rules-changelog.md) |
| Integration | Real device adapter | **Starter:** [integration.md](integration.md) + examples `append_observed`, `ingest_turn_off`; extend with MQTT/GPIO |

## Refocus

- **Operability**: traces, `explain`, snapshots are there — prioritize **perf measurements** or **real IO**.
- **Core**: **schema** and **custom rules** before UI / multi-node.
