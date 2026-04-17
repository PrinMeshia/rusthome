//! EPIC 1 — Observed truth vs Derived projection (reconciliation).

mod common;

use rusthome_app::{
    append_observed_light_fact, correction_for_observed_light, replay_state, ObservedLightAppend,
    RunLimits,
};
use rusthome_core::{
    ConfigSnapshot, Event, FactEvent, PhysicalProjectionMode, Provenance, State, StateView,
};
use rusthome_infra::{load_and_sort, verify_contiguous_sequence, Journal, JournalAppend};
use rusthome_rules::Registry;
use uuid::Uuid;

#[test]
fn correction_none_when_projection_matches_observed() {
    let state = State::new();
    assert!(correction_for_observed_light(&state, "r", true).is_none());
}

#[test]
fn derived_on_then_observed_off_emits_correction_and_final_state_off() {
    let (_dir, jpath) = common::temp_events_jsonl();
    let mut journal = Journal::open(&jpath).unwrap();
    let causal = Uuid::nil();

    journal
        .append(JournalAppend {
            timestamp: 1,
            causal_chain_id: causal,
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Fact(FactEvent::LightOn {
                room: "living".into(),
                provenance: Provenance::Derived,
            }),
        })
        .unwrap()
        .expect_committed();

    let mut state = replay_state(&jpath).unwrap();
    assert!(state.light_on("living"));
    assert_eq!(
        state.light_last_provenance("living"),
        Some(Provenance::Derived)
    );

    let registry = Registry::v0_default();
    registry.validate_boot().unwrap();
    let config = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };

    append_observed_light_fact(
        &mut journal,
        &mut state,
        &registry,
        &config,
        ObservedLightAppend {
            timestamp: 2,
            causal_chain_id: causal,
            room: "living".into(),
            on: false,
            correlation_id: None,
            trace_id: None,
        },
        RunLimits::default(),
        None,
    )
    .unwrap();

    assert!(!state.light_on("living"));
    assert_eq!(
        state.light_last_provenance("living"),
        Some(Provenance::Observed)
    );

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
    assert_eq!(entries.len(), 3);

    let n_corr = entries
        .iter()
        .filter(|e| {
            matches!(
                e.event,
                Event::Fact(FactEvent::StateCorrectedFromObservation { .. })
            )
        })
        .count();
    assert_eq!(n_corr, 1);

    let st2 = replay_state(&jpath).unwrap();
    assert_eq!(state, st2);
}
