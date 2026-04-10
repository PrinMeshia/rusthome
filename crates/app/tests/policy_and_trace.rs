//! §14.5 IoAnchored, §15 trace, §6.16 CommandIo (no-op state).

use rusthome_app::{ingest_observation_with_causal_traced, replay_state, RunLimits};
use rusthome_core::{
    apply_event, ConfigSnapshot, Event, FactEvent, ObservationEvent, PhysicalProjectionMode,
    Provenance, RunError, State, StateView,
};
use rusthome_infra::{load_and_sort, verify_contiguous_sequence, Journal, JournalAppend};
use rusthome_rules::Registry;
use uuid::Uuid;

#[test]
fn io_anchored_rejects_derived_light_from_rule() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let registry = Registry::v0_default();
    registry.validate_boot().unwrap();
    let config = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::IoAnchored,
        ..Default::default()
    };

    let mut journal = Journal::open(&jpath).unwrap();
    let mut state = State::new();
    let err = ingest_observation_with_causal_traced(
        &mut journal,
        &mut state,
        &registry,
        &config,
        1,
        ObservationEvent::MotionDetected {
            room: "hall".into(),
        },
        Uuid::new_v4(),
        RunLimits::default(),
        None,
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
    assert_eq!(
        n_err, 1,
        "EPIC 4: one ErrorOccurred line before returning Err"
    );
    let last = entries.last().expect("journal must not be empty");
    match &last.event {
        Event::ErrorOccurred(er) => {
            assert_eq!(er.error_type, "run.io_anchored_derived_actuator");
            assert!(er.context.contains("IoAnchored"));
        }
        _ => panic!(
            "last line must be ErrorOccurred, got {:?}",
            last.event
        ),
    }
}

#[test]
fn rule_trace_records_all_rules_per_event() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let registry = Registry::v0_default();
    registry.validate_boot().unwrap();
    let config = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };

    let mut journal = Journal::open(&jpath).unwrap();
    let mut state = State::new();
    let mut trace = Vec::new();
    ingest_observation_with_causal_traced(
        &mut journal,
        &mut state,
        &registry,
        &config,
        1,
        ObservationEvent::MotionDetected { room: "x".into() },
        Uuid::nil(),
        RunLimits::default(),
        Some(&mut trace),
    )
    .unwrap();

    let n_rules = registry.rules().len();
    let traces_root: Vec<_> = trace.iter().filter(|t| t.trigger_sequence == 0).collect();
    assert_eq!(
        traces_root.len(),
        n_rules,
        "one trace row per registry rule for the first journal line"
    );
    assert!(traces_root.iter().any(|t| t.rule_id == "R1" && t.matched));
    assert!(traces_root.iter().any(|t| t.rule_id == "R2" && t.matched));
    assert!(traces_root
        .iter()
        .any(|t| { t.rule_id == "R3" && !t.matched && t.reason == Some("not_subscribed".into()) }));
}

#[test]
fn command_io_fact_replay_is_state_noop() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&jpath).unwrap();
    journal
        .append(JournalAppend {
            timestamp: 1,
            causal_chain_id: Uuid::nil(),
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Fact(FactEvent::CommandIo {
                command_id: None,
                room: None,
                phase: rusthome_core::CommandIoPhase::Acked,
                provenance: Provenance::Observed,
            }),
        })
        .unwrap()
        .expect_committed();

    let st = replay_state(&jpath).unwrap();
    assert!(!st.light_on("any"));
    let s0 = State::new();
    let f = FactEvent::CommandIo {
        command_id: None,
        room: None,
        phase: rusthome_core::CommandIoPhase::Dispatched {
            logical_deadline: None,
        },
        provenance: Provenance::Derived,
    };
    let s1 = apply_event(&s0, &f).unwrap();
    assert_eq!(s1, s0);
}

#[test]
fn journal_line_is_canonical_sorted_keys() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&jpath).unwrap();
    journal
        .append(JournalAppend {
            timestamp: 2,
            causal_chain_id: Uuid::nil(),
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Observation(ObservationEvent::MotionDetected { room: "z".into() }),
        })
        .unwrap()
        .expect_committed();

    let raw = std::fs::read_to_string(&jpath).unwrap();
    let line = raw.lines().next().unwrap();
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    if let serde_json::Value::Object(map) = &v {
        let keys: Vec<_> = map.keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "top-level JSON keys must be sorted (§8.3)");
    } else {
        panic!("expected object");
    }

    let entries = load_and_sort(&jpath).unwrap();
    verify_contiguous_sequence(&entries).unwrap();
}
