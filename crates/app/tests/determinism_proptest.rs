//! §6.18 / plan §3 — same ingest sequence → same state and same journal (property tests).
//!
//! Includes **limited** §6.6 paths (`max_events_per_run`, `max_events_generated_per_root`):
//! early stop + `ErrorOccurred` remain reproducible.

use proptest::prelude::*;
use rusthome_app::{ingest_observation_with_causal, RunLimits};
use rusthome_core::{ConfigSnapshot, ObservationEvent, RunError, State};
use rusthome_infra::Journal;
use rusthome_rules::Registry;
use uuid::Uuid;

fn run_motion_chain(n: usize) -> (State, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::v0_default();
    reg.validate_boot().unwrap();
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits::default();
    for t in 0..n {
        let room = format!("room_{t}");
        ingest_observation_with_causal(
            &mut journal,
            &mut state,
            &reg,
            &cfg,
            t as i64,
            ObservationEvent::MotionDetected { room },
            Uuid::from_u128((t as u128).saturating_mul(0x10_01)),
            limits.clone(),
        )
        .unwrap();
    }
    let raw = std::fs::read_to_string(&path).unwrap();
    (state, raw)
}

/// Same V0 motion under §6.6 caps: journal and result identical across two runs.
fn run_motion_limited(max_events_per_run: u64, max_events_generated_per_root: u64) -> (State, String, Result<(), RunError>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = Registry::v0_default();
    reg.validate_boot().unwrap();
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits {
        max_events_per_run,
        max_events_generated_per_root,
        ..RunLimits::default()
    };
    let res = ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::MotionDetected {
            room: "limit-det-room".into(),
        },
        Uuid::from_u128(0x11A1_B2C3_0000_0001),
        limits,
    );
    let raw = std::fs::read_to_string(&path).unwrap();
    (state, raw, res)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn motion_sequence_is_byte_identical_across_runs(n in 1usize..24usize) {
        let (s1, j1) = run_motion_chain(n);
        let (s2, j2) = run_motion_chain(n);
        prop_assert_eq!(s1, s2);
        prop_assert_eq!(j1, j2);
    }

    /// §6.6 caps: depending on values, cascade stops on `MaxEventsPerRun` or
    /// `MaxEventsGeneratedPerRoot` (or completes) — always bit-identical reproducible.
    #[test]
    fn run_limits_cascade_is_deterministic(max_run in 2u64..200u64, max_gen in 1u64..30u64) {
        let (st1, j1, r1) = run_motion_limited(max_run, max_gen);
        let (st2, j2, r2) = run_motion_limited(max_run, max_gen);
        prop_assert_eq!(st1, st2);
        prop_assert_eq!(j1, j2);
        prop_assert_eq!(r1, r2);
    }
}
