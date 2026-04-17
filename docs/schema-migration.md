# Journal schema migration (sketch)

This document outlines how to evolve persisted data when adding **new event kinds** or changing the JSON shape of `JournalEntry` lines. The engine already carries a **per-line** `schema_version` (see `rusthome_core::SCHEMA_VERSION`, `MIN_SUPPORTED_JOURNAL_SCHEMA` in [`crates/core/src/journal.rs`](../crates/core/src/journal.rs)).

## What exists today

- Each JSONL line is a `JournalEntry` with `schema_version`, metadata (`timestamp`, `sequence`, …), and a flattened [`Event`](../crates/core/src/event.rs) body.
- Append always writes the **current** `SCHEMA_VERSION`.
- Load validates `schema_version` ∈ `[MIN_SUPPORTED_JOURNAL_SCHEMA, SCHEMA_VERSION]` before replay (see `validate_supported_schema`).
- Breaking changes are recorded in [rules-changelog.md](rules-changelog.md) alongside digest and rule-set notes.

## Bumping `SCHEMA_VERSION`

1. **Domain change**: add or adjust `Event` variants in `rusthome-core`, update serde tagging, and extend `apply_event` / validation as needed.
2. **Bump** `SCHEMA_VERSION` in `crates/core/src/journal.rs` and adjust `MIN_SUPPORTED_JOURNAL_SCHEMA` only if you intentionally **drop** support for old lines (avoid unless necessary).
3. **Tests**: extend `rusthome-infra` journal round-trip tests and any golden JSONL fixtures; run `cargo test --workspace`.
4. **Document** the change in [rules-changelog.md](rules-changelog.md) (what readers must know when mixing binaries).

## Offline migration of an existing file

When you need **one file** to move from era *A* → *B* without keeping dual-read forever:

1. Copy `events.jsonl` to a backup path.
2. Write a small tool (or one-off script) that:
   - reads each line with serde, tolerant of old `schema_version`;
   - maps old variants to new ones (or injects defaults);
   - re-emits lines with the new `schema_version` and **preserves** `sequence` / causal metadata **or** renumbers from 0 with a clear note that sequence identity changed (breaks references unless you also rewrite `parent_sequence` — usually prefer preserving order and sequences).
3. Run `rusthome snapshot` / `repair` as appropriate after migration (see [implementation.md](implementation.md) for current CLI).
4. Verify with `rusthome replay` / `rusthome state` on the migrated file.

## Forward compatibility

- Prefer **additive** JSON fields (optional serde fields) before a hard version bump.
- For new event families, consider a **feature flag** in `rusthome.toml` before enabling writes, so mixed deployments can upgrade readers first.

## Relation to rules presets

Changing **rules** (presets `v0`, `home`, …) does not by itself require a journal schema bump; changing **persisted event shapes** does. Keep the two concepts separate in release notes.
