# rusthome documentation

Index of files under `docs/` — avoid duplicating detail in the root [README](../README.md).

| Document | Contents |
| -------- | -------- |
| [implementation.md](implementation.md) | Code state, plan conformance table, crates, CLI, on-disk files, tests |
| [user-rules.md](user-rules.md) | Presets, `rusthome.toml`, extending `Rule` / `Registry`, DSL–WASM ideas |
| [onboarding-rules.md](onboarding-rules.md) | How to add a rule (boot, graph, §6.18) |
| [rules-changelog.md](rules-changelog.md) | Digests `rules-v0` / `rules-home` / `rules-minimal` and schema evolution |
| [errors.md](errors.md) | Taxonomy: domain / pipeline / infra errors (§14.6–14.8) |
| [reconciliation.md](reconciliation.md) | Journal ↔ world, Observed / Derived (§14.7) |
| [integration.md](integration.md) | Edge adapters, examples, `rusthome serve` / `rusthome-web` lab UI, CLI vs library |
| [io-lifecycle.md](io-lifecycle.md) | `CommandIo` cycle §6.16 |
| [perf-assumptions.md](perf-assumptions.md) | Load assumptions, bench, Pi measurements (§7.1) |
| [roadmap-2-semaines.md](roadmap-2-semaines.md) | Sprint history + **next steps** (short backlog) |

Example config: [../configs/rusthome.example.toml](../configs/rusthome.example.toml).
