//! `TurnOffLight` command + R7: journal append and projection.

mod common;

use rusthome_app::{
    ingest_command_with_causal, ingest_observation_with_causal, replay_state, ConfigSnapshot,
    RunError, RunLimits,
};
use rusthome_core::{
    CommandEvent, Event, ObservationEvent, PhysicalProjectionMode, State, StateView,
};
use rusthome_infra::{load_and_sort, verify_contiguous_sequence, Journal};
use rusthome_rules::Registry;
use uuid::Uuid;

#[test]
fn turn_off_after_motion_clears_light() {
    let (_dir, jpath) = common::temp_events_jsonl();
    let causal_on = Uuid::from_u128(0x70FF_0001);
    let causal_off = Uuid::from_u128(0x70FF_0002);
    let room = "kitchen".to_string();

    let registry = Registry::minimal_default();
    registry.validate_boot().unwrap();
    let config = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };
    let limits = RunLimits::default();

    let mut journal = Journal::open(&jpath).unwrap();
    let mut state = State::new();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &registry,
        &config,
        10,
        ObservationEvent::MotionDetected { room: room.clone() },
        causal_on,
        limits.clone(),
    )
    .unwrap();
    assert!(state.light_on(&room));

    let cmd_id = Uuid::from_u128(0x70FF_C0DE_0001);
    ingest_command_with_causal(
        &mut journal,
        &mut state,
        &registry,
        &config,
        20,
        CommandEvent::TurnOffLight {
            room: room.clone(),
            command_id: cmd_id,
        },
        causal_off,
        limits,
    )
    .unwrap();

    assert!(
        !state.light_on(&room),
        "R7 should apply LightOff for the room"
    );

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
    let st2 = replay_state(&jpath).unwrap();
    assert_eq!(st2, state);
}

#[test]
fn io_anchored_rejects_derived_light_off_from_r7() {
    let (_dir, jpath) = common::temp_events_jsonl();
    let room = "io-off".to_string();
    let causal_on = Uuid::from_u128(0x10FF_A001);
    let causal_off = Uuid::from_u128(0x10FF_A002);

    let registry = Registry::minimal_default();
    registry.validate_boot().unwrap();
    let sim = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };
    let anchored = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::IoAnchored,
        ..Default::default()
    };
    let limits = RunLimits::default();

    let mut journal = Journal::open(&jpath).unwrap();
    let mut state = State::new();
    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &registry,
        &sim,
        1,
        ObservationEvent::MotionDetected { room: room.clone() },
        causal_on,
        limits.clone(),
    )
    .unwrap();
    assert!(state.light_on(&room));

    let err = ingest_command_with_causal(
        &mut journal,
        &mut state,
        &registry,
        &anchored,
        2,
        CommandEvent::TurnOffLight {
            room: room.clone(),
            command_id: Uuid::from_u128(0x10FF_C001),
        },
        causal_off,
        limits,
    )
    .unwrap_err();

    assert!(
        matches!(err, RunError::IoAnchoredDerivedActuator(_)),
        "unexpected {err:?}"
    );

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
    let n_err = entries
        .iter()
        .filter(|e| matches!(e.event, Event::ErrorOccurred(_)))
        .count();
    assert_eq!(n_err, 1, "EPIC 4: ErrorOccurred before returning Err");
}
