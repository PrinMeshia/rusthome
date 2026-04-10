# Adding a rule (plan §6.9–§6.17, §6.12.1)

1. **Create** a struct implementing `Rule` under `crates/rules/src/` (e.g. `rules_impl.rs`).
2. **Set** `rule_id` (stable), `priority`, `namespaces`, `consumes`, `produces`.
3. **Implement** `eval` and metadata accessors (`rule_id`, `consumes` / `produces` as slices, `namespaces` → `Vec<&str>`): only from `event`, `ctx.state()` (via `StateView`), `ctx.config` — no files, no wall clock, no `std::env`.
4. **Register** the rule: either in a bundle used by a preset ([`bundle.rs`](../crates/rules/src/bundle.rs), [`preset.rs`](../crates/rules/src/preset.rs), `Registry::v0_default()` / `home_default()` / `minimal_default()`), or via `Registry::from_rules` for a custom registry.
5. **Run** `cargo test -p rusthome-rules` — boot checks cycles, §6.15, §6.17, `produces` consistency.
6. **Exceptional family transitions** (§6.17): if a rule needs a transition the default matrix forbids, add an entry to `Registry::family_transition_whitelist` (no “already allowed” duplicates — boot rejects redundant entries).
7. **Graph**: `cargo run -p rusthome-cli -- rules-doc` (Mermaid output).

## Oscillations (§6.18)

If two rules can **alternate** contradictory facts on the same axis (same room / same aggregate), the engine stays deterministic but may **hit §6.6 caps** without a clear domain diagnosis. Do **cross-review** and add invariant tests on reference scenarios. In code: `crates/app/tests/shared_axis_invariant.rs` and `determinism_proptest.rs`, plus unit tests for caps in `pipeline.rs` (`#[cfg(test)]`).
