# Errors: fatal vs recoverable (plan §14.8)

This document is an **initial taxonomy** for operations. Exact variants evolve with code (`RunError`, `ApplyError`, `JournalError`).

## Fatal (stop, intervene, or repair)


| Source                             | Example                                         | Typical action                                                |
| ---------------------------------- | ----------------------------------------------- | ------------------------------------------------------------- |
| `JournalError::Corrupt`            | Invalid JSON, truncated line                    | `rusthome repair` then analysis; restore from backup            |
| `JournalError::UnsupportedSchemaVersion` | Line `schema_version` outside **2..=3**   | Upgrade tool / migration, or restore backup; see [rules-changelog.md](rules-changelog.md) |
| `JournalError::SequenceMismatch`   | Duplicate or gap in `sequence`                  | Same path; journal inconsistent with model                    |
| `JournalError::TimestampRegressed` | Live append with logical time below last commit | Fix upstream (§3.4); no silent patch                        |
| Process crash / OOM                | —                                               | Restart; replay from journal (§14.2)                        |


## Domain / run (current V0 fail-fast)


| Error                                   | V0 behaviour                                                 | Possible evolution (§14.6)                                                 |
| ---------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------- |
| `ApplyError` (light already ON/OFF, etc.) | Run stops; rejected fact not appended                        | Append failure fact, dead letter, quarantine                               |
| `RunError::MaxEventsPerRun`              | Loop / logical explosion                                     | Tighten rules, caps, review graph §6.13                                    |
| `RunError::MaxEventsGeneratedPerRoot`    | Same §6.6.2                                                  | Same                                                                       |
| `RunError::RunTimeBudgetExceeded`        | `Instant` budget exceeded                                    | Adjust `max_wall_ms_per_run` or rules                                      |
| `RunError::QueueCapacityExceeded`        | FIFO too large                                               | Same family                                                                |
| `RunError::IoAnchoredDerivedActuator`    | Rule emitted **Derived** light fact in **IoAnchored**        | Use **Simulation** for demo, or implement §6.16 path + Observed            |


## EPIC 4 — `ErrorOccurred` (journal audit)

On several **FIFO drain** failure paths (caps, time budget, `ApplyError` on a dequeued fact, IoAnchored, `append` failure, etc.), the pipeline **best-effort** appends an **`ErrorOccurred`** line with stable `error_type` (`apply.*`, `run.*`) and short textual `context`.

- **Replay / projection**: `replay_state` and `apply_event` **ignore** `ErrorOccurred` — only **facts** mutate state; an error line does not “undo” a prior fact.
- **Rules**: the **Error** family is not in any allowed registry transition; rules do not consume these events.
- **Append paradox**: if appending `ErrorOccurred` fails (disk full, corruption, etc.), the original error is still returned to the caller; audit may be missing despite the failure.
- **Outside drain**: errors **before** or **outside** the drain (e.g. failed observation append) do not go through this hook and may not produce `ErrorOccurred`.

See `schema_version` 3 in [rules-changelog.md](rules-changelog.md).


## Journal vs physical world (§14.4)

A journal-level “resolved” error **does not guarantee** the real world matches: see [reconciliation.md](reconciliation.md).
