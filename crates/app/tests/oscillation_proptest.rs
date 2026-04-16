//! §6.18 — multi-rule oscillation property tests (graph).
//!
//! Extends `determinism_proptest.rs` with **richer rule graphs**: deeper cascades,
//! competing rules on the same trigger, and tight `RunLimits`.  Verifies that
//! `drain_fifo` converges or hits caps **deterministically** (same inputs → identical
//! journal, state, and error).

use std::sync::Arc;

use proptest::prelude::*;
use rusthome_app::{ingest_observation_with_causal, RunLimits};
use rusthome_core::{
    deterministic_command_id, CommandEvent, ConfigSnapshot, Event, EventKind, FactEvent,
    ObservationEvent, Rule, RuleContext, RunError, State, StateView,
};
use rusthome_infra::Journal;
use rusthome_rules::{Registry, RegistryError};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Synthetic rules — deep cascade (on + off in a single motion)
// ---------------------------------------------------------------------------

/// LightOn → TurnOffLight: auto-toggle (lower priority than R4 so log fires first).
struct RToggle;

impl Rule for RToggle {
    fn rule_id(&self) -> &str {
        "R_toggle"
    }
    fn priority(&self) -> i32 {
        -1
    }
    fn consumes(&self) -> &[EventKind] {
        &[EventKind::LightOn]
    }
    fn produces(&self) -> &[EventKind] {
        &[EventKind::TurnOffLight]
    }
    fn namespaces(&self) -> Vec<&str> {
        vec!["lighting"]
    }
    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Fact(FactEvent::LightOn { room, .. }) => {
                let command_id = deterministic_command_id(
                    "R_toggle",
                    "turn_off_light",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    room,
                );
                vec![Event::Command(CommandEvent::TurnOffLight {
                    room: room.clone(),
                    command_id,
                })]
            }
            _ => vec![],
        }
    }
}

/// LightOff → LogUsage: mirrors R4 for the off path.
struct RLogOff;

impl Rule for RLogOff {
    fn rule_id(&self) -> &str {
        "R_logoff"
    }
    fn priority(&self) -> i32 {
        0
    }
    fn consumes(&self) -> &[EventKind] {
        &[EventKind::LightOff]
    }
    fn produces(&self) -> &[EventKind] {
        &[EventKind::LogUsage]
    }
    fn namespaces(&self) -> Vec<&str> {
        vec!["logging"]
    }
    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Fact(FactEvent::LightOff { room, .. }) => {
                let item = format!("light_off:{room}");
                let command_id = deterministic_command_id(
                    "R_logoff",
                    "log_usage",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    &item,
                );
                vec![Event::Command(CommandEvent::LogUsage { item, command_id })]
            }
            _ => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Synthetic rule — creates a cycle (for boot-rejection test)
// ---------------------------------------------------------------------------

/// LogUsage → TurnOnLight: combined with R3 (TurnOnLight→LightOn) and R4 (LightOn→LogUsage),
/// this closes the loop TurnOnLight → LightOn → LogUsage → TurnOnLight.
struct CyclicRule;

impl Rule for CyclicRule {
    fn rule_id(&self) -> &str {
        "CyclicRule"
    }
    fn priority(&self) -> i32 {
        0
    }
    fn consumes(&self) -> &[EventKind] {
        &[EventKind::LogUsage]
    }
    fn produces(&self) -> &[EventKind] {
        &[EventKind::TurnOnLight]
    }
    fn namespaces(&self) -> Vec<&str> {
        vec!["lighting"]
    }
    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Command(CommandEvent::LogUsage { item, .. }) => {
                let command_id = deterministic_command_id(
                    "CyclicRule",
                    "turn_on_light",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    item,
                );
                vec![Event::Command(CommandEvent::TurnOnLight {
                    room: item.clone(),
                    command_id,
                })]
            }
            _ => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Home preset extended with RToggle + RLogOff, keeping only lighting/logging rules.
///
/// One motion produces a full on→off cycle per room (~13 journal lines),
/// so repeated motions in the same room succeed (light ends up off).
fn deep_registry() -> Registry {
    let base = Registry::home_default();
    let lighting_ids = ["R1", "R3", "R4", "R5", "R7"];
    let mut rules: Vec<Arc<dyn Rule>> = base
        .rules()
        .iter()
        .filter(|r| lighting_ids.contains(&r.rule_id()))
        .cloned()
        .collect();
    rules.push(Arc::new(RToggle));
    rules.push(Arc::new(RLogOff));
    Registry::from_rules(rules, &[])
}

fn run_deep_chain(
    rooms: &[String],
    limits: RunLimits,
) -> (State, String, Vec<Result<(), RunError>>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = deep_registry();
    reg.validate_boot().unwrap();
    let cfg = ConfigSnapshot::default();
    let mut results = Vec::new();
    for (i, room) in rooms.iter().enumerate() {
        let res = ingest_observation_with_causal(
            &mut journal,
            &mut state,
            &reg,
            &cfg,
            i as i64,
            ObservationEvent::MotionDetected { room: room.clone() },
            Uuid::from_u128((i as u128).wrapping_mul(0x1001).wrapping_add(1)),
            limits.clone(),
        );
        results.push(res);
    }
    let raw = std::fs::read_to_string(&path).unwrap();
    (state, raw, results)
}

fn run_deep_chain_limited(
    max_run: u64,
    max_gen: u64,
) -> (State, String, Result<(), RunError>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut journal = Journal::open(&path).unwrap();
    let mut state = State::new();
    let reg = deep_registry();
    reg.validate_boot().unwrap();
    let cfg = ConfigSnapshot::default();
    let limits = RunLimits {
        max_events_per_run: max_run,
        max_events_generated_per_root: max_gen,
        ..RunLimits::default()
    };
    let res = ingest_observation_with_causal(
        &mut journal,
        &mut state,
        &reg,
        &cfg,
        0,
        ObservationEvent::MotionDetected {
            room: "deep-limit".into(),
        },
        Uuid::from_u128(0xDE_E9_0001),
        limits,
    );
    let raw = std::fs::read_to_string(&path).unwrap();
    (state, raw, res)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[test]
fn deep_registry_passes_boot_validation() {
    let reg = deep_registry();
    reg.validate_boot().unwrap();
}

#[test]
fn deep_single_motion_produces_on_off_cycle() {
    let rooms = vec!["hall".to_string()];
    let (state, journal, results) = run_deep_chain(&rooms, RunLimits::default());
    assert!(results.iter().all(|r| r.is_ok()), "cascade must succeed");
    assert!(
        !state.light_on("hall"),
        "R_toggle turns the light back off within the same cascade"
    );
    let line_count = journal.lines().filter(|l| !l.is_empty()).count();
    assert!(
        line_count >= 10,
        "deep cascade must produce at least 10 journal lines, got {line_count}"
    );
}

#[test]
fn deep_repeated_same_room_succeeds() {
    let rooms = vec!["kitchen".into(), "kitchen".into(), "kitchen".into()];
    let (_state, _journal, results) = run_deep_chain(&rooms, RunLimits::default());
    for (i, r) in results.iter().enumerate() {
        assert!(
            r.is_ok(),
            "motion {i} in same room must succeed (light cycles on→off each time)"
        );
    }
}

#[test]
fn boot_rejects_cyclic_event_graph() {
    let base = Registry::home_default();
    let lighting_ids = ["R1", "R3", "R4", "R5", "R7"];
    let mut rules: Vec<Arc<dyn Rule>> = base
        .rules()
        .iter()
        .filter(|r| lighting_ids.contains(&r.rule_id()))
        .cloned()
        .collect();
    rules.push(Arc::new(CyclicRule));
    let reg = Registry::from_rules(rules, &[]);
    let err = reg.validate_boot().expect_err("cyclic graph must be rejected");
    assert!(
        matches!(err, RegistryError::CycleDetected),
        "expected CycleDetected, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Deep cascade across multiple rooms: journal and state byte-identical across two runs.
    #[test]
    fn deep_chain_multi_room_determinism(n_rooms in 1usize..5, n_motions in 1usize..10) {
        let rooms: Vec<String> = (0..n_rooms).map(|i| format!("room_{i}")).collect();
        let sequence: Vec<String> = (0..n_motions).map(|i| rooms[i % n_rooms].clone()).collect();
        let limits = RunLimits::default();

        let (s1, j1, r1) = run_deep_chain(&sequence, limits.clone());
        let (s2, j2, r2) = run_deep_chain(&sequence, limits);

        prop_assert_eq!(&s1, &s2);
        prop_assert_eq!(&j1, &j2);
        prop_assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            prop_assert_eq!(a, b);
        }
    }

    /// Deep cascade with tight RunLimits: whether the run succeeds or hits a cap,
    /// the outcome is identical across two runs.
    #[test]
    fn deep_chain_tight_limits_determinism(max_run in 3u64..60, max_gen in 2u64..30) {
        let (s1, j1, r1) = run_deep_chain_limited(max_run, max_gen);
        let (s2, j2, r2) = run_deep_chain_limited(max_run, max_gen);

        prop_assert_eq!(&s1, &s2);
        prop_assert_eq!(&j1, &j2);
        prop_assert_eq!(&r1, &r2);
    }

    /// Repeated motions in one room with the deep registry: each on→off cycle
    /// leaves the room off, so the next motion succeeds. Deterministic across runs.
    #[test]
    fn deep_chain_repeated_single_room(n_repeats in 1usize..8) {
        let rooms: Vec<String> = (0..n_repeats).map(|_| "repeat-room".into()).collect();
        let limits = RunLimits::default();

        let (s1, j1, r1) = run_deep_chain(&rooms, limits.clone());
        let (s2, j2, r2) = run_deep_chain(&rooms, limits);

        prop_assert_eq!(&s1, &s2);
        prop_assert_eq!(&j1, &j2);
        for (a, b) in r1.iter().zip(r2.iter()) {
            prop_assert_eq!(a, b);
        }
    }
}
