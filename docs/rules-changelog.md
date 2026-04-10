# Rules changelog (plan §20)

| Version / digest | Change |
|------------------|--------|
| `rules-v0` (default snapshot digest if preset `v0`) | Registry R1–R5: motion → light → usage log; R3 in simulation → `LightOn` + `CommandIo` Dispatched (deadline) + `CommandIo` Acked (EPIC 2). |
| `rules-home` (default digest if preset `home`) | **R1 + R3 + R4 + R5**: light + IO + usage log, **without** R2 (`NotifyUser`). Digest string unchanged; behaviour **differs** from `v0` on one motion (one fewer command). |
| `rules-minimal` (default digest if preset `minimal`) | R1 + R3 only: motion → light + IO, no notify or usage log. |
| Journal schema | Fact `StateCorrectedFromObservation` + last light provenance per room (`State.light_last_provenance`) for reconciliation §14.7. |
| `schema_version` 2 | Commands require `command_id`; append dedup (`JournalAppendOutcome`). |
| `schema_version` 3 | `ErrorOccurred` event (audit pipeline failures in the drain); ignored by facts-only replay. |

Update this on every behavioural registry change. The snapshot `rules_digest` field should reflect the version tracked in prod or lab.
