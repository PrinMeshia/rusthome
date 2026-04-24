//! Three event families — plan §3.5.
//!
//! The module is split into subfiles: [`types`] (common enums), fact/command/observation/error
//! payloads, and [`envelope`] for the top-level persisted `Event` and `EventKind`. All variants
//! stay available in every build so historical JSONL journals (schemas 2–5) always deserialize.

mod command;
mod envelope;
mod error_event;
mod fact;
mod observation;
mod types;

pub use command::CommandEvent;
pub use envelope::{Event, EventKind};
pub use error_event::ErrorOccurredEvent;
pub use fact::FactEvent;
pub use observation::ObservationEvent;
pub use types::{CommandIoPhase, LightActuatorState, Provenance};
