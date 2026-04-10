use crate::error::ApplyError;
use crate::event::{CommandIoPhase, FactEvent, Provenance};
use crate::reducer::{apply_event, validate_fact_for_append};
use crate::state::State;
use crate::view::StateView;

#[test]
fn light_on_then_validate_duplicate_fails() {
    let s0 = State::new();
    let f = FactEvent::LightOn {
        room: "hall".into(),
        provenance: Provenance::Derived,
    };
    validate_fact_for_append(&s0, &f).unwrap();
    let s1 = apply_event(&s0, &f).unwrap();
    let r = validate_fact_for_append(&s1, &f);
    assert_eq!(r, Err(ApplyError::LightAlreadyOn("hall".into())));
}

#[test]
fn command_io_phase_deserializes_legacy_succeeded_tag() {
    let j = r#"{"phase":"succeeded"}"#;
    let p: CommandIoPhase = serde_json::from_str(j).expect("alias acked");
    assert_eq!(p, CommandIoPhase::Acked);
}

#[test]
fn command_io_phase_deserializes_dispatched_without_deadline() {
    let j = r#"{"phase":"dispatched"}"#;
    let p: CommandIoPhase = serde_json::from_str(j).expect("dispatched without fields");
    assert_eq!(
        p,
        CommandIoPhase::Dispatched {
            logical_deadline: None
        }
    );
}

#[test]
fn usage_logged_applies() {
    let s0 = State::new();
    let f = FactEvent::UsageLogged {
        item: "light".into(),
        provenance: Provenance::Derived,
    };
    let s1 = apply_event(&s0, &f).unwrap();
    assert_eq!(s1.last_log_item(), Some("light"));
}
