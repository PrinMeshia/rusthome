//! `minimal` registry (R1+R3): motion turns light on without notify / usage-log chain.

mod common;

use rusthome_app::{ingest_observation_with_causal, RunLimits};
use rusthome_core::{ConfigSnapshot, ObservationEvent, State, StateView};
use rusthome_infra::Journal;
use rusthome_rules::RulesPreset;
use uuid::Uuid;

#[test]
fn minimal_motion_light_on_without_usage_log() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = RulesPreset::Minimal.load_registry().unwrap();
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits::default();
    let room = "only-light".to_string();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::MotionDetected {
            room: room.clone(),
        },
        Uuid::from_u128(0x4D1E_0001),
        limits,
    )
    .unwrap();

    assert!(state.light_on(&room));
    assert_eq!(state.last_log_item(), None);
}
