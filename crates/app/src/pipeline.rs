//! FIFO pipeline: dequeue → fact apply or rules → append synthetics → enqueue (plan §6).

use std::collections::{HashSet, VecDeque};
use std::time::Instant;

use rusthome_core::{
    apply_event, validate_fact_for_append, ApplyError, ErrorOccurredEvent, Event, FactEvent,
    HostRuntimeConfig, PhysicalProjectionMode, Provenance, RuleContext, State,
};
use rusthome_journal::JournalEntry;
use rusthome_rules::Registry;

use rusthome_infra::{Journal, JournalAppend, JournalAppendOutcome};

use crate::{RuleEvaluationRecord, RunError};

/// Append `ErrorOccurred` before surfacing the error (best-effort; if that append fails too, ignore — bootstrap paradox §8.1).
fn append_pipeline_error_best_effort(
    journal: &mut Journal,
    trigger: &JournalEntry,
    err: &RunError,
) {
    let ts = journal
        .last_timestamp_committed
        .map_or(trigger.timestamp, |lt| lt.max(trigger.timestamp));
    let event = Event::ErrorOccurred(ErrorOccurredEvent {
        error_type: err.stable_type_id(),
        context: err.to_string(),
    });
    let _ = journal.append(JournalAppend {
        timestamp: ts,
        causal_chain_id: trigger.causal_chain_id,
        parent_sequence: Some(trigger.sequence),
        parent_event_id: trigger.event_id,
        rule_id: None,
        event_id: None,
        correlation_id: trigger.correlation_id,
        trace_id: trigger.trace_id,
        event,
    });
}

fn map_apply_with_audit(
    journal: &mut Journal,
    entry: &JournalEntry,
    r: Result<State, ApplyError>,
) -> Result<State, RunError> {
    r.map_err(|ae| {
        let re = RunError::Apply(ae);
        append_pipeline_error_best_effort(journal, entry, &re);
        re
    })
}

#[derive(Debug, Clone)]
pub struct RunLimits {
    pub max_events_per_run: u64,
    pub max_events_generated_per_root: u64,
    pub max_wall_ms_per_run: u64,
    pub max_pending_events: usize,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_events_per_run: 10_000,
            max_events_generated_per_root: 500,
            max_wall_ms_per_run: 30_000,
            max_pending_events: 50_000,
        }
    }
}

struct Emission {
    rule_id: String,
    priority: i32,
    action_ordinal: usize,
    event: Event,
}

/// §14.5 — in IoAnchored, forbid appending a derived "physical" actuator fact without an IO path.
fn check_io_anchored_emission(event: &Event, config: &dyn HostRuntimeConfig) -> Result<(), RunError> {
    if config.physical_projection_mode() != PhysicalProjectionMode::IoAnchored {
        return Ok(());
    }
    match event {
        Event::Fact(FactEvent::LightOn {
            provenance: Provenance::Derived,
            ..
        })
        | Event::Fact(FactEvent::LightOff {
            provenance: Provenance::Derived,
            ..
        }) => Err(RunError::io_anchored_derived_actuator(
            "LightOn/LightOff Derived in IoAnchored — use Observed + §6.16 path",
        )),
        _ => Ok(()),
    }
}

fn collect_emissions_and_traces(
    registry: &Registry,
    entry: &JournalEntry,
    config: &dyn HostRuntimeConfig,
    state: &State,
) -> (Vec<Emission>, Vec<RuleEvaluationRecord>) {
    let ctx = RuleContext {
        state,
        config,
        trigger_timestamp: entry.timestamp,
        causal_chain_id: entry.causal_chain_id,
        parent_sequence: Some(entry.sequence),
        parent_event_id: entry.event_id,
    };
    let trigger = &entry.event;
    let mut out = Vec::new();
    let mut traces = Vec::new();
    for r in registry.rules() {
        let subscribed = r.consumes().contains(&trigger.kind());
        if !subscribed {
            traces.push(RuleEvaluationRecord {
                trigger_sequence: entry.sequence,
                trigger_kind: trigger.kind(),
                rule_id: r.rule_id().to_string(),
                matched: false,
                reason: Some("not_subscribed".into()),
            });
            continue;
        }
        let produced = r.eval(trigger, &ctx);
        let matched = !produced.is_empty();
        traces.push(RuleEvaluationRecord {
            trigger_sequence: entry.sequence,
            trigger_kind: trigger.kind(),
            rule_id: r.rule_id().to_string(),
            matched,
            reason: if matched {
                None
            } else {
                Some("evaluated_empty".into())
            },
        });
        for (ord, ev) in produced.into_iter().enumerate() {
            let k = ev.kind();
            if !r.produces().contains(&k) {
                debug_assert!(
                    r.produces().contains(&k),
                    "rule {} emitted {:?}",
                    r.rule_id(),
                    k
                );
            }
            out.push(Emission {
                rule_id: r.rule_id().to_string(),
                priority: r.priority(),
                action_ordinal: ord,
                event: ev,
            });
        }
    }
    out.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.rule_id.cmp(&b.rule_id))
            .then_with(|| a.action_ordinal.cmp(&b.action_ordinal))
    });
    (out, traces)
}

/// Process a FIFO of journal entries (first item already persisted). Appends all synthetics.
///
/// `rule_trace`: if `Some`, records one line per registry rule (plan §15).
///
/// §6.6.2: synthetic appends whose `parent_sequence` is in the causality tree of the **first**
/// queue event at drain start count toward `max_events_generated_per_root`.
pub fn drain_fifo(
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &dyn HostRuntimeConfig,
    mut queue: VecDeque<JournalEntry>,
    limits: RunLimits,
    mut rule_trace: Option<&mut Vec<RuleEvaluationRecord>>,
) -> Result<(), RunError> {
    let start = Instant::now();
    let mut processed: u64 = 0;
    let mut generated_from_root: u64 = 0;
    let root_seq = queue.front().map(|e| e.sequence);
    let mut causal_tree: HashSet<u64> = root_seq.into_iter().collect();

    while let Some(entry) = queue.pop_front() {
        if start.elapsed().as_millis() > limits.max_wall_ms_per_run as u128 {
            let err =
                RunError::run_time_budget(start.elapsed().as_millis(), limits.max_wall_ms_per_run);
            append_pipeline_error_best_effort(journal, &entry, &err);
            return Err(err);
        }

        processed += 1;
        if processed > limits.max_events_per_run {
            let err = RunError::max_events_per_run(processed, limits.max_events_per_run);
            append_pipeline_error_best_effort(journal, &entry, &err);
            return Err(err);
        }

        if queue.len() > limits.max_pending_events {
            let err = RunError::queue_capacity(queue.len(), limits.max_pending_events);
            append_pipeline_error_best_effort(journal, &entry, &err);
            return Err(err);
        }

        if let Event::Fact(f) = &entry.event {
            *state = map_apply_with_audit(journal, &entry, apply_event(state, f))?;
        }

        let (emissions, traces) = collect_emissions_and_traces(registry, &entry, config, state);
        if let Some(buf) = rule_trace.as_mut() {
            buf.extend(traces);
        }

        let mut shadow = state.clone();
        for em in emissions {
            if let Err(e) = check_io_anchored_emission(&em.event, config) {
                append_pipeline_error_best_effort(journal, &entry, &e);
                return Err(e);
            }
            if let Event::Fact(ref fact) = em.event {
                validate_fact_for_append(&shadow, fact).map_err(|ae| {
                    let re = RunError::Apply(ae);
                    append_pipeline_error_best_effort(journal, &entry, &re);
                    re
                })?;
                shadow = map_apply_with_audit(journal, &entry, apply_event(&shadow, fact))?;
            }
            let outcome = match journal.append(JournalAppend {
                timestamp: entry.timestamp,
                causal_chain_id: entry.causal_chain_id,
                parent_sequence: Some(entry.sequence),
                parent_event_id: entry.event_id,
                rule_id: Some(em.rule_id.clone()),
                event_id: None,
                correlation_id: entry.correlation_id,
                trace_id: entry.trace_id,
                event: em.event,
            }) {
                Ok(o) => o,
                Err(je) => {
                    let re = RunError::journal(je.to_string());
                    append_pipeline_error_best_effort(journal, &entry, &re);
                    return Err(re);
                }
            };
            let new_entry = match outcome {
                JournalAppendOutcome::Committed(e) => e,
                JournalAppendOutcome::DuplicateCommandSkipped { .. } => continue,
            };

            if let Some(ps) = new_entry.parent_sequence {
                if causal_tree.contains(&ps) {
                    causal_tree.insert(new_entry.sequence);
                    generated_from_root += 1;
                    if generated_from_root > limits.max_events_generated_per_root {
                        let err = RunError::max_events_generated_per_root(
                            generated_from_root,
                            limits.max_events_generated_per_root,
                        );
                        append_pipeline_error_best_effort(journal, &entry, &err);
                        return Err(err);
                    }
                }
            }
            queue.push_back(new_entry);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use rusthome_core::{Event, ObservationEvent, State};

    use crate::{ConfigSnapshot, RunError};
    use rusthome_infra::{Journal, JournalAppend};
    use rusthome_rules::Registry;
    use uuid::Uuid;

    use super::{drain_fifo, JournalEntry, RunLimits};

    fn motion_entry(journal: &mut Journal, room: &str) -> JournalEntry {
        journal
            .append(JournalAppend {
                timestamp: 0,
                causal_chain_id: Uuid::from_u128(0xC0FFEE),
                parent_sequence: None,
                parent_event_id: None,
                rule_id: None,
                event_id: None,
                correlation_id: None,
                trace_id: None,
                event: Event::Observation(ObservationEvent::MotionDetected { room: room.into() }),
            })
            .unwrap()
            .expect_committed()
    }

    #[test]
    fn drain_fifo_max_events_per_run_stops_cascade() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut journal = Journal::open(&path).unwrap();
        let reg = Registry::v0_default();
        reg.validate_boot().unwrap();
        let config = ConfigSnapshot::default();
        let mut state = State::new();
        let entry = motion_entry(&mut journal, "lab");
        let queue = VecDeque::from([entry]);
        let limits = RunLimits {
            max_events_per_run: 1,
            ..RunLimits::default()
        };
        let err = drain_fifo(&mut journal, &mut state, &reg, &config, queue, limits, None)
            .expect_err("second dequeue should exceed max_events_per_run");
        assert!(matches!(err, RunError::MaxEventsPerRun { .. }));
    }

    #[test]
    fn drain_fifo_max_events_generated_per_root_stops_deep_cascade() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut journal = Journal::open(&path).unwrap();
        let reg = Registry::v0_default();
        reg.validate_boot().unwrap();
        let config = ConfigSnapshot::default();
        let mut state = State::new();
        let entry = motion_entry(&mut journal, "lab");
        let queue = VecDeque::from([entry]);
        let limits = RunLimits {
            max_events_generated_per_root: 1,
            ..RunLimits::default()
        };
        let err = drain_fifo(&mut journal, &mut state, &reg, &config, queue, limits, None)
            .expect_err("V0 cascade appends more than one synthetic from root");
        assert!(matches!(err, RunError::MaxEventsGeneratedPerRoot { .. }));
    }
}
