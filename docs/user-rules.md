# User rules (extending the engine)

Today the **V0** demo ships light / log rules in `rusthome-rules` (`R1`–`R5`). The engine (`rusthome-app`) does not hard-code those types: it takes a boot-validated `Registry`.

## Already decoupled

- **Pipeline**: `drain_fifo` / `ingest_*` take `&Registry` and rules via `Arc` in the registry.
- **Contract**: the [`Rule`](../crates/core/src/rules.rs) trait in `rusthome-core` — pure `eval` (no IO, no wall clock).
- **Extension**: `Registry::from_rules` (`../crates/rules/src/registry.rs`) accepts `Vec<Arc<dyn Rule>>`; `Registry::v0_default()` is only a bundled preset.
- **CLI / config**: presets `v0` (R1–R5), `home` (R1+R3+R4+R5, no R2 notify), `minimal` (R1+R3) — details in [implementation.md](implementation.md#cli-rusthome) and [configs/rusthome.example.toml](../configs/rusthome.example.toml). New preset: [`preset.rs`](../crates/rules/src/preset.rs) + [`bundle.rs`](../crates/rules/src/bundle.rs) / `Registry::…_default()` + `RulesPreset::default_rules_digest`.

## Adding rules in Rust (recommended for V0+)

1. Create a binary or lib crate (e.g. `rusthome-rules-home`) depending on `rusthome-core`.
2. Implement `Rule` with owned fields if needed (`id: String`, `consumes: Vec<EventKind>`, …) and return `&str` / slices borrowed from `self`.
3. Build `Registry::from_rules(vec![Arc::new(MyRule), …], &[])` then `validate_boot()`.
4. Wire the CLI (new preset in `preset.rs` + `load_registry`) or a future service to load that registry instead of built-in presets.

Existing guards still apply: acyclic graph §6.13, family transitions §6.17, `produces` consistent with `eval` §6.12.1.

## Further options (not in current code)

| Approach | Pros | Cons |
|----------|------|------|
| **Declarative DSL** (JSON/YAML → limited rules) | No compile step; non-Rust users | Restricted language; interpreter to maintain |
| **`.so` plugins + stable ABI** | Arbitrary Rust/C logic | Security, ABI versioning |
| **WASM** (mentioned in README) | Sandbox | Runtime, size, complexity |

## Hard-coded “light”

`LightOn` / `LightOff` / `TurnOnLight` are **event model** variants in `rusthome-core`. Decoupling from the *domain* (thermostat, shutters, …) is a separate effort: grow `Event` / `FactEvent` or add a generic subsystem (e.g. typed entities) without breaking existing journals (`schema_version` migration).

For custom **business** rules without changing the schema yet, the immediate lever remains: **new `Rule` implementations** + **registry loaded at startup**.
