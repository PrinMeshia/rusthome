//! rusthome-core — pure domain: events, state, reducer, rule trait (plan §4, §3, §6.10).
//!
//! No wall clock, no IO.

pub mod command_id;
pub mod config;
pub mod error;
pub mod event;
pub mod journal;
pub mod reducer;
pub mod rules;
pub mod state;
pub mod trace;
pub mod view;

pub use command_id::{deterministic_command_id, COMMAND_ID_NAMESPACE};
pub use config::{ConfigSnapshot, PhysicalProjectionMode};
pub use error::{ApplyError, JournalSchemaError, RunError};
pub use event::{
    CommandEvent, CommandIoPhase, ErrorOccurredEvent, Event, EventKind, FactEvent,
    LightActuatorState, ObservationEvent, Provenance,
};
pub use journal::{
    journal_schema_supported, JournalEntry, MIN_SUPPORTED_JOURNAL_SCHEMA, SCHEMA_VERSION,
};
pub use reducer::{apply_event, validate_fact_for_append};
pub use rules::{Rule, RuleContext};
pub use state::State;
pub use trace::RuleEvaluationRecord;
pub use view::StateView;

#[cfg(test)]
mod reducer_tests;
