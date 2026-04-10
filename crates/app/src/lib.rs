//! Orchestration — pipeline + helpers.
//!
//! §14.1 — replay and live processing are **not idempotent**: the same fact replayed twice
//! applies `apply_event` twice until dedup applies.

mod pipeline;
mod reconciliation;

pub use pipeline::{drain_fifo, RunLimits};
pub use reconciliation::{
    append_observed_light_fact, correction_for_observed_light, ObservedLightAppend,
};

use std::collections::VecDeque;
use std::path::Path;

use rusthome_core::{
    apply_event, CommandEvent, ConfigSnapshot, Event, ObservationEvent, RuleEvaluationRecord,
    RunError, State,
};
use rusthome_infra::{verify_contiguous_sequence, Journal, JournalAppend, JournalAppendOutcome};
use rusthome_rules::Registry;
use uuid::Uuid;

/// Ingest external observation: append, then drain cascade (`causation_id` chosen by caller).
#[allow(clippy::too_many_arguments)]
pub fn ingest_observation_with_causal(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    timestamp: i64,
    observation: ObservationEvent,
    causal_chain_id: Uuid,
    limits: RunLimits,
) -> Result<(), RunError> {
    ingest_observation_with_causal_traced(
        journal,
        state,
        registry,
        config,
        timestamp,
        observation,
        causal_chain_id,
        limits,
        None,
    )
}

/// Like [`ingest_observation_with_causal`] with optional §15 trace.
#[allow(clippy::too_many_arguments)]
pub fn ingest_observation_with_causal_traced(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    timestamp: i64,
    observation: ObservationEvent,
    causal_chain_id: Uuid,
    limits: RunLimits,
    rule_trace: Option<&mut Vec<RuleEvaluationRecord>>,
) -> Result<(), RunError> {
    let entry = match journal
        .append(JournalAppend {
            timestamp,
            causal_chain_id,
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Observation(observation),
        })
        .map_err(|e| RunError::journal(e.to_string()))?
    {
        JournalAppendOutcome::Committed(e) => e,
        JournalAppendOutcome::DuplicateCommandSkipped { .. } => {
            unreachable!("observation is not a command")
        }
    };
    let mut q = VecDeque::new();
    q.push_back(entry);
    drain_fifo(journal, state, registry, config, q, limits, rule_trace)?;
    Ok(())
}

/// Append a **Command** line, then drain the FIFO (new `causal_chain_id` unless you use the `_with_causal` variant).
#[allow(clippy::too_many_arguments)]
pub fn ingest_command_with_causal_traced(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    timestamp: i64,
    command: CommandEvent,
    causal_chain_id: Uuid,
    limits: RunLimits,
    rule_trace: Option<&mut Vec<RuleEvaluationRecord>>,
) -> Result<(), RunError> {
    let entry = match journal
        .append(JournalAppend {
            timestamp,
            causal_chain_id,
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Command(command),
        })
        .map_err(|e| RunError::journal(e.to_string()))?
    {
        JournalAppendOutcome::Committed(e) => e,
        JournalAppendOutcome::DuplicateCommandSkipped { .. } => {
            return Ok(());
        }
    };
    let mut q = VecDeque::new();
    q.push_back(entry);
    drain_fifo(journal, state, registry, config, q, limits, rule_trace)?;
    Ok(())
}

/// Like [`ingest_command_with_causal_traced`] without trace buffer.
#[allow(clippy::too_many_arguments)]
pub fn ingest_command_with_causal(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    timestamp: i64,
    command: CommandEvent,
    causal_chain_id: Uuid,
    limits: RunLimits,
) -> Result<(), RunError> {
    ingest_command_with_causal_traced(
        journal,
        state,
        registry,
        config,
        timestamp,
        command,
        causal_chain_id,
        limits,
        None,
    )
}

pub fn ingest_command(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    timestamp: i64,
    command: CommandEvent,
    limits: RunLimits,
) -> Result<(), RunError> {
    ingest_command_with_causal(
        journal,
        state,
        registry,
        config,
        timestamp,
        command,
        Uuid::new_v4(),
        limits,
    )
}

/// Ingest external observation: append, then drain cascade (new random `causation_id`).
pub fn ingest_observation(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    timestamp: i64,
    observation: ObservationEvent,
    limits: RunLimits,
) -> Result<(), RunError> {
    ingest_observation_with_causal(
        journal,
        state,
        registry,
        config,
        timestamp,
        observation,
        Uuid::new_v4(),
        limits,
    )
}

/// Replay: apply facts in journal order only (plan §4 — state changes only via facts).
/// Commands/observations are ignored for projection; they are already reflected by appended facts.
///
/// §14.1 — two replays of the same journal yield the same state, but **re-injecting** the same
/// events into a **new** run without dedup guards can double effects.
pub fn replay_state(path: &Path) -> Result<State, RunError> {
    let entries =
        rusthome_infra::load_and_sort(path).map_err(|e| RunError::journal(e.to_string()))?;
    verify_contiguous_sequence(&entries).map_err(|e| RunError::journal(e.to_string()))?;
    let mut state = State::new();
    for e in entries {
        if let Event::Fact(ref f) = e.event {
            state = apply_event(&state, f).map_err(RunError::Apply)?;
        }
    }
    Ok(state)
}
