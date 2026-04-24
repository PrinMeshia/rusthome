//! rusthome-core — pure domain: events, state, reducer, rule trait (plan §4, §3, §6.10).
//!
//! The [`event`](event) module is split into subfiles (`event/types`, `fact`, `command`, etc.) for
//! structure; the persisted `Event` shape is unchanged (full journal compatibility). The **journal
//! line** envelope is in the `rusthome_journal` crate. **Process config** (TOML) is
//! `rusthome_app::ConfigSnapshot` implementing [`HostRuntimeConfig`]; **deterministic command ids** for
//! rules are in `rusthome_rules`.
//!
//! No wall clock, no IO.

pub mod config;
pub mod error;
pub mod host_runtime_config;
pub mod event;
pub mod reducer;
pub mod rules;
pub mod state;
pub mod view;

pub use config::PhysicalProjectionMode;
pub use error::ApplyError;
pub use host_runtime_config::{DefaultHostConfig, HostRuntimeConfig};
pub use event::{
    CommandEvent, CommandIoPhase, ErrorOccurredEvent, Event, EventKind, FactEvent,
    LightActuatorState, ObservationEvent, Provenance,
};
pub use reducer::{apply_event, validate_fact_for_append};
pub use rules::{Rule, RuleContext};
pub use state::State;
pub use view::StateView;

#[cfg(test)]
mod reducer_tests;
