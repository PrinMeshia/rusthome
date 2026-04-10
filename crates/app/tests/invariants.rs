//! §6.18 — projection / journal invariants (anti-drift).

use rusthome_core::{apply_event, ApplyError, Event, FactEvent, Provenance, State};
use rusthome_infra::{load_and_sort, verify_contiguous_sequence, JournalAppend};
use uuid::Uuid;

#[test]
fn replay_fails_on_contradictory_duplicate_light_on() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let mut journal = rusthome_infra::Journal::open(&jpath).unwrap();
    let f = FactEvent::LightOn {
        room: "x".into(),
        provenance: Provenance::Derived,
    };
    for _ in 0..2 {
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
                event: Event::Fact(f.clone()),
            })
            .unwrap()
            .expect_committed();
    }

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
    let mut state = State::new();
    for e in entries {
        if let Event::Fact(ref fact) = e.event {
            match apply_event(&state, fact) {
                Ok(s) => state = s,
                Err(ApplyError::LightAlreadyOn(room)) => {
                    assert_eq!(room, "x");
                    return;
                }
                Err(e) => panic!("unexpected {e:?}"),
            }
        }
    }
    panic!("expected second LightOn to fail with LightAlreadyOn");
}
