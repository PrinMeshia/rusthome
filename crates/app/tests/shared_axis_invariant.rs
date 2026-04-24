//! §6.18 — rules sharing the same trigger / state axis: errors and explicit bounds.

mod common;

use rusthome_app::RunError;
use rusthome_app::{ingest_observation_with_causal, RunLimits};
use rusthome_app::ConfigSnapshot;
use rusthome_core::{ApplyError, ObservationEvent, State};
use rusthome_infra::Journal;
use rusthome_rules::Registry;
use uuid::Uuid;

/// Two motions in the same room: V0 cascade applies two derived `LightOn` → deterministic business failure.
#[test]
fn second_motion_same_room_is_light_already_on() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::v0_default();
    reg.validate_boot().unwrap();
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits::default();
    let room = "shared-axis-room".to_string();
    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::MotionDetected { room: room.clone() },
        Uuid::from_u128(1),
        limits.clone(),
    )
    .unwrap();

    let err = ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        1,
        ObservationEvent::MotionDetected { room: room.clone() },
        Uuid::from_u128(2),
        limits,
    )
    .expect_err("second LightOn for same room must fail apply");

    match err {
        RunError::Apply(ApplyError::LightAlreadyOn(r)) => assert_eq!(r, room),
        other => panic!("expected LightAlreadyOn, got {other:?}"),
    }
}

/// R1 and R2 both consume `MotionDetected`: one observation yields multiple commands; result stays reproducible (stable line count for V0 registry).
#[test]
fn dual_subscribers_fixed_journal_shape() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::v0_default();
    reg.validate_boot().unwrap();
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits::default();
    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::MotionDetected {
            room: "dual-sub".into(),
        },
        Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_00D0_0A11),
        limits,
    )
    .unwrap();

    let lines = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .count();
    // 1 obs + 2 commands + R3 facts (3) + R4 command + R5 fact = 8 (deterministic pipeline order)
    assert_eq!(
        lines, 8,
        "V0 default registry: expect fixed cascade depth for one motion"
    );
}
