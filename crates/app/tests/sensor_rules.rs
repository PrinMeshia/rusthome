//! Integration test: R8–R11 and R12–R13 for temperature, contact, and humidity events.

mod common;

use std::path::Path;

use rusthome_app::{ingest_observation_with_causal, RunLimits};
use rusthome_app::ConfigSnapshot;
use rusthome_core::{ObservationEvent, State, StateView};
use rusthome_infra::Journal;
use rusthome_rules::Registry;
use uuid::Uuid;

fn line_count(path: &Path) -> usize {
    match std::fs::read_to_string(path) {
        Ok(s) => s.lines().filter(|l| !l.is_empty()).count(),
        Err(_) => 0,
    }
}

#[test]
fn temperature_reading_records_and_logs() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::TemperatureReading {
            sensor_id: "living-room".into(),
            millidegrees_c: 21500,
        },
        Uuid::from_u128(0x7E_0001),
        RunLimits::default(),
    )
    .unwrap();

    assert_eq!(
        state.temperature("living-room"),
        Some(21500),
        "temperature should be recorded in state"
    );
    assert_eq!(
        state.last_log_item(),
        Some("temperature:living-room"),
        "R10 should log the temperature reading"
    );

    // Cascade: 1 obs + R8 fact + R10 cmd + R5 usage_logged = 4 lines
    assert_eq!(line_count(&path), 4, "temperature cascade shape");
}

#[test]
fn humidity_reading_records_and_logs() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::HumidityReading {
            sensor_id: "bath".into(),
            permille_rh: 655,
        },
        Uuid::from_u128(0x7E_0002),
        RunLimits::default(),
    )
    .unwrap();

    assert_eq!(
        state.humidity_permille("bath"),
        Some(655),
        "humidity should be recorded in state"
    );
    assert_eq!(
        state.last_log_item(),
        Some("humidity:bath"),
        "R13 should log the humidity reading"
    );

    // Cascade: 1 obs + R12 fact + R13 cmd + R5 usage_logged = 4 lines
    assert_eq!(line_count(&path), 4, "humidity cascade shape");
}

#[test]
fn contact_changed_records_and_logs() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::ContactChanged {
            sensor_id: "front-door".into(),
            open: true,
        },
        Uuid::from_u128(0xC0AC_0001),
        RunLimits::default(),
    )
    .unwrap();

    assert_eq!(
        state.contact_open("front-door"),
        Some(true),
        "contact should be recorded as open"
    );
    assert_eq!(
        state.last_log_item(),
        Some("contact:front-door"),
        "R11 should log the contact change"
    );

    // Cascade: 1 obs + R9 fact + R11 cmd + R5 usage_logged = 4 lines
    assert_eq!(line_count(&path), 4, "contact cascade shape");
}

#[test]
fn temperature_updates_overwrite_previous() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();

    for (i, millidegrees) in [18000, 19500, 21000].iter().enumerate() {
        ingest_observation_with_causal(
            &mut journal,
            &mut state,
            &reg,
            &cfg,
            i as i64,
            ObservationEvent::TemperatureReading {
                sensor_id: "kitchen".into(),
                millidegrees_c: *millidegrees,
            },
            Uuid::from_u128(0x7E_0100 + i as u128),
            RunLimits::default(),
        )
        .unwrap();
    }

    assert_eq!(
        state.temperature("kitchen"),
        Some(21000),
        "last temperature wins"
    );
}

#[test]
fn contact_toggles() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::home_default();
    let cfg = ConfigSnapshot::default();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::ContactChanged {
            sensor_id: "window".into(),
            open: true,
        },
        Uuid::from_u128(0xC0AC_0010),
        RunLimits::default(),
    )
    .unwrap();
    assert_eq!(state.contact_open("window"), Some(true));

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        1,
        ObservationEvent::ContactChanged {
            sensor_id: "window".into(),
            open: false,
        },
        Uuid::from_u128(0xC0AC_0011),
        RunLimits::default(),
    )
    .unwrap();
    assert_eq!(
        state.contact_open("window"),
        Some(false),
        "contact now closed"
    );
}

#[test]
fn minimal_preset_records_sensor_facts_without_logging() {
    let (_dir, path) = common::temp_events_jsonl();
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::minimal_default();
    let cfg = ConfigSnapshot::default();

    ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::TemperatureReading {
            sensor_id: "outdoor".into(),
            millidegrees_c: -5300,
        },
        Uuid::from_u128(0x01_0001),
        RunLimits::default(),
    )
    .unwrap();

    assert_eq!(state.temperature("outdoor"), Some(-5300));
    assert!(
        state.last_log_item().is_none(),
        "minimal preset has no R10/R11 logging"
    );
    // Cascade: 1 obs + R8 fact = 2 lines (no logging in minimal)
    assert_eq!(line_count(&path), 2, "minimal temperature cascade shape");
}
