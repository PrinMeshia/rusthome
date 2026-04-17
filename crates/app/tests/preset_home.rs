//! `home` preset: R1+R3+R4+R5 (usage log present, **no** `NotifyUser` / R2).

mod common;

use std::path::Path;

use rusthome_app::{ingest_observation_with_causal, RunLimits};
use rusthome_core::{ConfigSnapshot, ObservationEvent, State, StateView};
use rusthome_infra::Journal;
use rusthome_rules::RulesPreset;
use uuid::Uuid;

fn line_count(path: &Path) -> usize {
    match std::fs::read_to_string(path) {
        Ok(s) => s.lines().filter(|l| !l.is_empty()).count(),
        Err(_) => 0,
    }
}

#[test]
fn home_one_motion_journal_shorter_than_v0() {
    let room = "living".to_string();
    let causal = Uuid::from_u128(0xB0DE_0001);
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits::default();

    let (_dir_h, path_h) = common::temp_events_jsonl();
    let mut journal_h = Journal::open(&path_h).unwrap();
    let mut state_h = State::new();
    let reg_h = RulesPreset::Home.load_registry().unwrap();
    ingest_observation_with_causal(
        &mut journal_h,
        &mut state_h,
        &reg_h,
        &cfg,
        0,
        ObservationEvent::MotionDetected {
            room: room.clone(),
        },
        causal,
        limits.clone(),
    )
    .unwrap();

    let (_dir_v, path_v) = common::temp_events_jsonl();
    let mut journal_v = Journal::open(&path_v).unwrap();
    let mut state_v = State::new();
    let reg_v = RulesPreset::V0.load_registry().unwrap();
    ingest_observation_with_causal(
        &mut journal_v,
        &mut state_v,
        &reg_v,
        &cfg,
        0,
        ObservationEvent::MotionDetected {
            room: room.clone(),
        },
        causal,
        limits,
    )
    .unwrap();

    let n_h = line_count(&path_h);
    let n_v = line_count(&path_v);
    assert!(
        n_h < n_v,
        "home (no R2) must produce fewer lines than v0; home={n_h} v0={n_v}"
    );
    // 1 obs + 1 cmd (TurnOnLight) + R3 (3 faits) + cmd LogUsage + UsageLogged = 7
    assert_eq!(n_h, 7, "home journal shape for one motion");
    assert_eq!(n_v, 8, "v0 journal shape for one motion (dual subscribers)");

    assert!(state_h.light_on(&room));
    assert!(state_v.light_on(&room));
    assert!(state_h.last_log_item().is_some());
    assert!(state_v.last_log_item().is_some());
}
