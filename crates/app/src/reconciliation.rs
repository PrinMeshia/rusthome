//! System truth: Observed wins over a Derived projection (reconciliation §14.7).

use std::collections::VecDeque;

use rusthome_core::{
    validate_fact_for_append, Event, FactEvent, LightActuatorState, Provenance, State, StateView,
};
use rusthome_infra::{Journal, JournalAppend};
use rusthome_rules::Registry;
use uuid::Uuid;

use crate::pipeline::drain_fifo;
use crate::{ConfigSnapshot, RuleEvaluationRecord, RunError, RunLimits};

/// Parameters for an **Observed** `LightOn` / `LightOff` append (not from rules).
#[derive(Debug, Clone)]
pub struct ObservedLightAppend {
    pub timestamp: i64,
    pub causal_chain_id: Uuid,
    pub room: String,
    /// `true` = LightOn, `false` = LightOff.
    pub on: bool,
    pub correlation_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
}

fn actuator_expected(state: &State, room: &str) -> LightActuatorState {
    if state.light_on(room) {
        LightActuatorState::On
    } else {
        LightActuatorState::Off
    }
}

fn actuator_observed(on: bool) -> LightActuatorState {
    if on {
        LightActuatorState::On
    } else {
        LightActuatorState::Off
    }
}

/// If projection was based on a **Derived** fact and contradicts the observation, returns the audit fact.
pub fn correction_for_observed_light(state: &State, room: &str, on: bool) -> Option<FactEvent> {
    if state.light_last_provenance(room) != Some(Provenance::Derived) {
        return None;
    }
    let expected = actuator_expected(state, room);
    let observed = actuator_observed(on);
    if expected == observed {
        return None;
    }
    Some(FactEvent::StateCorrectedFromObservation {
        entity_id: room.to_string(),
        expected,
        observed,
        provenance: Provenance::Derived,
    })
}

/// Append **Observed** `LightOn`/`LightOff`; if diverging from Derived projection, append
/// `StateCorrectedFromObservation` first, then the observed fact; enqueue all for `drain_fifo`.
pub fn append_observed_light_fact(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    params: ObservedLightAppend,
    limits: RunLimits,
    rule_trace: Option<&mut Vec<RuleEvaluationRecord>>,
) -> Result<(), RunError> {
    let ObservedLightAppend {
        timestamp,
        causal_chain_id,
        room,
        on,
        correlation_id,
        trace_id,
    } = params;

    let fact = if on {
        FactEvent::LightOn {
            room: room.clone(),
            provenance: Provenance::Observed,
        }
    } else {
        FactEvent::LightOff {
            room: room.clone(),
            provenance: Provenance::Observed,
        }
    };

    validate_fact_for_append(state, &fact).map_err(RunError::Apply)?;

    let mut queue = VecDeque::new();

    if let Some(corr) = correction_for_observed_light(state, &room, on) {
        validate_fact_for_append(state, &corr).map_err(RunError::Apply)?;
        let corr_entry = journal
            .append(JournalAppend {
                timestamp,
                causal_chain_id,
                parent_sequence: None,
                parent_event_id: None,
                rule_id: None,
                event_id: None,
                correlation_id,
                trace_id,
                event: Event::Fact(corr),
            })
            .map_err(|e| RunError::journal(e.to_string()))?
            .expect_committed();
        queue.push_back(corr_entry);
    }

    let light_entry = journal
        .append(JournalAppend {
            timestamp,
            causal_chain_id,
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            event_id: None,
            correlation_id,
            trace_id,
            event: Event::Fact(fact),
        })
        .map_err(|e| RunError::journal(e.to_string()))?
        .expect_committed();
    queue.push_back(light_entry);

    drain_fifo(journal, state, registry, config, queue, limits, rule_trace)?;
    Ok(())
}
