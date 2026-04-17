//! Plan §16 — MotionDetected cascade: order, sequences, projection.

mod common;

use std::collections::VecDeque;

use rusthome_app::{drain_fifo, ingest_observation_with_causal, replay_state, RunLimits};
use rusthome_core::{
    ConfigSnapshot, Event, EventKind, ObservationEvent, PhysicalProjectionMode, State, StateView,
};
use rusthome_infra::{load_and_sort, verify_contiguous_sequence, Journal, JournalAppend};
use rusthome_rules::Registry;
use uuid::Uuid;

#[test]
fn scenario_16_journal_order_and_state() {
    let (_dir, jpath) = common::temp_events_jsonl();
    let causal = Uuid::from_u128(0x0016_0000_0000_0000_0000_0000_0000_0001);

    let registry = Registry::v0_default();
    registry.validate_boot().unwrap();
    let config = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };

    let mut journal = Journal::open(&jpath).unwrap();
    let mut state = State::new();
    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &registry,
        &config,
        10,
        ObservationEvent::MotionDetected {
            room: "hall".into(),
        },
        causal,
        RunLimits::default(),
    )
    .unwrap();

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
    assert_eq!(
        entries.len(),
        8,
        "§16 + R5 + R3: CommandIo Dispatched + Acked (EPIC 2) = 8 lines"
    );
    for (i, e) in entries.iter().enumerate() {
        assert_eq!(e.sequence, i as u64);
        assert_eq!(e.timestamp, 10);
        assert_eq!(e.causal_chain_id, causal);
    }

    let st = replay_state(&jpath).unwrap();
    assert!(st.light_on("hall"));
    assert_eq!(st.last_log_item(), Some("light:hall"));

    // Second replay identical
    assert_eq!(replay_state(&jpath).unwrap(), st);
}

#[test]
fn replay_does_not_mutate_journal() {
    let (_dir, jpath) = common::temp_events_jsonl();
    std::fs::write(&jpath, "").unwrap();
    let before = std::fs::read_to_string(&jpath).unwrap();
    replay_state(&jpath).unwrap();
    assert_eq!(std::fs::read_to_string(&jpath).unwrap(), before);
}

#[test]
fn fact_then_rules_light_on_triggers_log_usage() {
    let (_dir, jpath) = common::temp_events_jsonl();
    let registry = Registry::v0_default();
    registry.validate_boot().unwrap();
    let config = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };
    let causal = Uuid::nil();

    let mut journal = Journal::open(&jpath).unwrap();
    let mut state = State::new();

    let root = journal
        .append(JournalAppend {
            timestamp: 1,
            causal_chain_id: causal,
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Fact(rusthome_core::FactEvent::LightOn {
                room: "r1".into(),
                provenance: rusthome_core::Provenance::Derived,
            }),
        })
        .unwrap()
        .expect_committed();

    let mut q = VecDeque::new();
    q.push_back(root);
    drain_fifo(
        &mut journal,
        &mut state,
        &registry,
        &config,
        q,
        RunLimits::default(),
        None,
    )
    .unwrap();

    let entries = load_and_sort(&jpath).unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e.event.kind() == EventKind::LogUsage),
        "R4 should emit LogUsage after LightOn fact"
    );
}
