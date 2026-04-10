# Roadmap — history and next steps

> The old “two-week” plan (canonical JSON, trace §15, IO §6.16, `errors.md`, Pi bench, `rules-doc`, etc.) is **largely done**; current detail is in [implementation.md](implementation.md).

## Next steps (short backlog)

| Priority | Topic | Notes |
| -------- | ----- | ----- |
| Tests | Property §6.18 (§6.6 caps, determinism) | Done in `determinism_proptest`; **multi-rule** oscillations (graph) still open |
| Perf | §7.1 — p95 under load on Pi | `bench`, `scripts/bench-p95.sh`, [perf-assumptions.md](perf-assumptions.md) |
| Rules | `home` preset ≠ `v0` | **Done** — `arc_rules_home()` = R1+R3+R4+R5 (no R2) |
| Schema | New event types (beyond light) | `schema_version`, journal migration |
| Integration | Real device adapter | **Starter:** [integration.md](integration.md) + `rusthome-app` example `append_observed`; extend with MQTT/GPIO |

## Refocus

- **Operability**: traces, `explain`, snapshots are there — prioritize **perf measurements** or **real IO**.
- **Core**: **schema** and **custom rules** before UI / multi-node.
