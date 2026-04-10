//! EPIC 4 — `ErrorOccurred` ignored by replay (facts-only projection).

use rusthome_app::replay_state;
use rusthome_core::{ErrorOccurredEvent, Event, FactEvent, Provenance, StateView};
use rusthome_infra::{load_and_sort, verify_contiguous_sequence, Journal, JournalAppend};
use uuid::Uuid;

#[test]
fn replay_skips_error_occurred_lines() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&jpath).unwrap();

    journal
        .append(JournalAppend {
            timestamp: 1,
            causal_chain_id: Uuid::nil(),
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::ErrorOccurred(ErrorOccurredEvent {
                error_type: "test.fake".into(),
                context: "audit only".into(),
            }),
        })
        .unwrap()
        .expect_committed();

    journal
        .append(JournalAppend {
            timestamp: 1,
            causal_chain_id: Uuid::nil(),
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Fact(FactEvent::LightOn {
                room: "z".into(),
                provenance: Provenance::Derived,
            }),
        })
        .unwrap()
        .expect_committed();

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
    let state = replay_state(&jpath).unwrap();
    assert!(state.light_on("z"));
}
