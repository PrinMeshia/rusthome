//! Closed-loop integration: MQTT observation topics → `dispatch_mqtt_publish` → rules → derived state.
//!
//! Complements `mqtt_command_ingest` tests (commands). See `docs/scenarios.md` for operator-facing
//! walkthroughs.

mod common;

use rusthome_app::integrations::mqtt::{dispatch_mqtt_publish, wall_millis};
use rusthome_app::RunLimits;
use rusthome_app::ConfigSnapshot;
use rusthome_core::{State, StateView};
use rusthome_infra::Journal;
use rusthome_rules::Registry;

#[test]
fn motion_via_mqtt_turns_on_light_home_preset() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();
    let mut last_ts = wall_millis();

    let out = dispatch_mqtt_publish(
        "sensors/motion/living",
        b"",
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        RunLimits::default(),
        &mut last_ts,
    )
    .unwrap();

    assert!(out.is_some(), "motion topic should ingest; got {out:?}");
    assert!(
        state.light_on("living"),
        "home preset R1 should turn on light for room from topic"
    );
}

#[test]
fn motion_then_command_replay_matches() {
    use rusthome_app::replay_state;

    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();
    let mut last_ts = wall_millis();

    dispatch_mqtt_publish(
        "sensors/motion/kitchen",
        br#"{"room":"kitchen"}"#,
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        RunLimits::default(),
        &mut last_ts,
    )
    .unwrap();
    assert!(state.light_on("kitchen"));

    dispatch_mqtt_publish(
        "commands/light/kitchen/off",
        b"",
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        RunLimits::default(),
        &mut last_ts,
    )
    .unwrap();
    assert!(!state.light_on("kitchen"));

    let replayed = replay_state(&path).unwrap();
    assert_eq!(state.light_on("kitchen"), replayed.light_on("kitchen"));
}
