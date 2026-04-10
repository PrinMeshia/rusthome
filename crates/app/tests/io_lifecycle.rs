//! EPIC 2 — CommandIo ordering, logical deadline, max 1 retry after Timeout.

use rusthome_core::{
    apply_event, ApplyError, ConfigSnapshot, FactEvent, PhysicalProjectionMode, Provenance, State,
};

#[test]
fn ack_without_open_dispatch_rejected_for_room_key() {
    let state = State::new();
    let f = FactEvent::CommandIo {
        command_id: None,
        room: Some("lab".into()),
        phase: rusthome_core::CommandIoPhase::Acked,
        provenance: Provenance::Observed,
    };
    let err = apply_event(&state, &f).unwrap_err();
    assert!(matches!(
        err,
        ApplyError::IoTerminalWithoutOpenDispatch(ref k) if k.contains("lab")
    ));
}

#[test]
fn timeout_then_redispatch_once_then_block() {
    let mut state = State::new();

    let d = |s: &State, ts: i64| {
        apply_event(
            s,
            &FactEvent::CommandIo {
                command_id: None,
                room: Some("lab".into()),
                phase: rusthome_core::CommandIoPhase::Dispatched {
                    logical_deadline: Some(ts + 10),
                },
                provenance: Provenance::Observed,
            },
        )
    };
    let t = |s: &State| {
        apply_event(
            s,
            &FactEvent::CommandIo {
                command_id: None,
                room: Some("lab".into()),
                phase: rusthome_core::CommandIoPhase::Timeout,
                provenance: Provenance::Observed,
            },
        )
    };
    state = d(&state, 1).unwrap();
    state = t(&state).unwrap();
    state = d(&state, 20).unwrap();
    state = t(&state).unwrap();
    // Two timeouts without Ack: no further Dispatched allowed for this key.
    let err = d(&state, 30).unwrap_err();
    assert!(matches!(err, ApplyError::IoRetryBudgetExhausted(_)));
}

#[test]
fn config_carries_io_timeout_delta_default() {
    let c = ConfigSnapshot {
        physical_projection_mode: PhysicalProjectionMode::Simulation,
        ..Default::default()
    };
    assert_eq!(c.io_timeout_logical_delta, 60);
}
