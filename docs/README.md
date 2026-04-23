# rusthome documentation

Index of files under `docs/` — avoid duplicating detail in the root [README](../README.md).

| Document | Contents |
| -------- | -------- |
| [implementation.md](implementation.md) | Code state, plan conformance table, crates, CLI, on-disk files, tests |
| [user-rules.md](user-rules.md) | Presets, `rusthome.toml`, extending `Rule` / `Registry`, DSL–WASM ideas |
| [onboarding-rules.md](onboarding-rules.md) | How to add a rule (boot, graph, §6.18) |
| [rules-changelog.md](rules-changelog.md) | Digests `rules-v0` / `rules-home` / `rules-minimal` and schema evolution |
| [schema-migration.md](schema-migration.md) | Sketch: bumping `schema_version`, offline JSONL migration |
| [errors.md](errors.md) | Taxonomy: domain / pipeline / infra errors (§14.6–14.8) |
| [reconciliation.md](reconciliation.md) | Journal ↔ world, Observed / Derived (§14.7) |
| [integration.md](integration.md) | Edge adapters, examples, `rusthome serve` / `rusthome-web` lab UI, CLI vs library |
| [adapters-and-bridges.md](adapters-and-bridges.md) | Core vs bridges, `rusthome-bridge`, Z2M → contrat MQTT, ops / TLS / versioning |
| [zigbee-conbee.md](zigbee-conbee.md) | Clé Conbee / Zigbee : Zigbee2MQTT, broker partagé, appairage, mapping MQTT |
| [web-proxy.md](web-proxy.md) | TLS + reverse proxy (Caddy / nginx) in front of the lab web UI |
| [io-lifecycle.md](io-lifecycle.md) | `CommandIo` cycle §6.16 |
| [perf-assumptions.md](perf-assumptions.md) | Load assumptions, bench, Pi measurements (§7.1) |
| [testing-core.md](testing-core.md) | Regression checklist: determinism proptests, bench/MQTT scripts |
| [mqtt-contract.md](mqtt-contract.md) | Versioned MQTT topic/payload contract (`mqtt_ingest`) |
| [scenarios.md](scenarios.md) | Three concrete operator scenarios (motion, commands, Observed) |
| [ops-data-dir.md](ops-data-dir.md) | Backups, journal, systemd pointers, security reminder |
| [presence-bridge.md](presence-bridge.md) | Optional BLE presence → MQTT/journal (after core stable) |
| [roadmap-2-semaines.md](roadmap-2-semaines.md) | Sprint history + **next steps** (short backlog) |

Example config: [../configs/rusthome.example.toml](../configs/rusthome.example.toml). Web proxy examples: [../configs/Caddyfile.rusthome.example](../configs/Caddyfile.rusthome.example), [../configs/nginx-rusthome.conf.example](../configs/nginx-rusthome.conf.example).
