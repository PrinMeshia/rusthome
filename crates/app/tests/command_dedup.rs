//! EPIC 3 — append dedup on `command_id`.

use rusthome_core::{CommandEvent, Event};
use rusthome_infra::{Journal, JournalAppend, JournalAppendOutcome};
use uuid::Uuid;

#[test]
fn second_append_same_command_id_skips_line() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&jpath).unwrap();
    let cid = Uuid::from_u128(0xdead_beef_0001);
    let base = JournalAppend {
        timestamp: 1,
        causal_chain_id: Uuid::nil(),
        parent_sequence: None,
        parent_event_id: None,
        rule_id: None,
        event_id: None,
        correlation_id: None,
        trace_id: None,
        event: Event::Command(CommandEvent::TurnOnLight {
            room: "r".into(),
            command_id: cid,
        }),
    };
    let o1 = journal.append(base.clone()).unwrap();
    assert!(matches!(o1, JournalAppendOutcome::Committed(_)));
    let o2 = journal
        .append(JournalAppend {
            timestamp: 2,
            ..base.clone()
        })
        .unwrap();
    assert!(matches!(
        o2,
        JournalAppendOutcome::DuplicateCommandSkipped { command_id } if command_id == cid
    ));
    assert_eq!(journal.next_sequence, 1);

    let mut journal2 = Journal::open(&jpath).unwrap();
    let o3 = journal2.append(base).unwrap();
    assert!(matches!(
        o3,
        JournalAppendOutcome::DuplicateCommandSkipped { command_id } if command_id == cid
    ));
}
