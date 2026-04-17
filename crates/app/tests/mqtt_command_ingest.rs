//! Integration test: MQTT command topics → journal pipeline.

mod common;

use std::path::Path;

use rusthome_app::mqtt_ingest::{dispatch_mqtt_publish, wall_millis};
use rusthome_app::replay_state;
use rusthome_app::RunLimits;
use rusthome_core::{ConfigSnapshot, State, StateView};
use rusthome_infra::Journal;
use rusthome_rules::Registry;

fn line_count(path: &Path) -> usize {
    match std::fs::read_to_string(path) {
        Ok(s) => s.lines().filter(|l| !l.is_empty()).count(),
        Err(_) => 0,
    }
}

#[test]
fn turn_on_via_mqtt_topic() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();
    let mut last_ts = wall_millis();

    let result = dispatch_mqtt_publish(
        "commands/light/hall/on",
        b"",
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        RunLimits::default(),
        &mut last_ts,
    )
    .unwrap();

    assert!(result.is_some(), "command should be recognized");
    assert!(
        state.light_on("hall"),
        "hall light should be on after TurnOnLight"
    );
    assert!(line_count(&path) > 0, "journal should have entries");
}

#[test]
fn turn_off_via_mqtt_after_on() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();
    let mut last_ts = wall_millis();

    dispatch_mqtt_publish(
        "commands/light/kitchen/on",
        b"",
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

    assert!(
        !state.light_on("kitchen"),
        "kitchen light should be off after TurnOffLight"
    );
}

#[test]
fn unknown_command_topic_skipped() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();
    let mut last_ts = wall_millis();

    let result = dispatch_mqtt_publish(
        "commands/thermostat/living/set",
        b"22000",
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        RunLimits::default(),
        &mut last_ts,
    )
    .unwrap();

    assert!(result.is_none(), "unknown command topic should be skipped");
    assert_eq!(line_count(&path), 0);
}

#[test]
fn replay_after_mqtt_commands_is_deterministic() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();
    let mut last_ts = wall_millis();

    dispatch_mqtt_publish(
        "commands/light/garage/on",
        b"",
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        RunLimits::default(),
        &mut last_ts,
    )
    .unwrap();

    let replayed = replay_state(&path).unwrap();
    assert_eq!(
        state.light_on("garage"),
        replayed.light_on("garage"),
        "replay must match live state"
    );
}
