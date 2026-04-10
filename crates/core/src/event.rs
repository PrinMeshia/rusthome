//! Three event families — plan §3.5.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Epistemic tag on facts — plan §3.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Observed,
    Derived,
}

/// Physical state of a light (Observed vs projection reconciliation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightActuatorState {
    On,
    Off,
}

/// Command IO journal cycle (EPIC 2): **Command** = issued intent; `CommandIo` facts = `Dispatched` → terminal.
/// No `Acked` without a prior `Dispatched` (validated via `State` + shadow pipeline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum CommandIoPhase {
    /// Handed to driver / bus; `logical_deadline` = logical time §3 for watchdog / `Timeout`.
    Dispatched {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        logical_deadline: Option<i64>,
    },
    /// Hardware success (replaces legacy `succeeded` JSON).
    #[serde(alias = "succeeded")]
    Acked,
    Failed {
        reason: String,
    },
    Timeout,
}

/// Facts — only family that flows through `apply_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum FactEvent {
    LightOn {
        room: String,
        provenance: Provenance,
    },
    LightOff {
        room: String,
        provenance: Provenance,
    },
    UsageLogged {
        item: String,
        provenance: Provenance,
    },
    /// Command IO cycle — plan §6.16 (does not change lights in V0).
    CommandIo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command_id: Option<Uuid>,
        /// Target room (V0) — lifecycle tracking key if `command_id` is absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room: Option<String>,
        phase: CommandIoPhase,
        provenance: Provenance,
    },
    /// Logs projection vs observation divergence (truth = Observed) — reconciliation §14.7.
    StateCorrectedFromObservation {
        entity_id: String,
        expected: LightActuatorState,
        observed: LightActuatorState,
        provenance: Provenance,
    },
}

/// Commands — intentions, never applied directly to state.
/// `command_id` required (EPIC 3) — use `deterministic_command_id` in rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum CommandEvent {
    TurnOnLight { room: String, command_id: Uuid },
    NotifyUser { command_id: Uuid },
    LogUsage { item: String, command_id: Uuid },
}

impl CommandEvent {
    pub fn command_id(&self) -> Uuid {
        match self {
            CommandEvent::TurnOnLight { command_id, .. }
            | CommandEvent::NotifyUser { command_id }
            | CommandEvent::LogUsage { command_id, .. } => *command_id,
        }
    }
}

/// Observations — external signals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum ObservationEvent {
    MotionDetected { room: String },
}

/// Persisted runtime error (EPIC 4) — no-op for `replay_state` / `apply_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorOccurredEvent {
    pub error_type: String,
    pub context: String,
}

/// Top-level persisted event envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Event {
    Fact(FactEvent),
    Command(CommandEvent),
    Observation(ObservationEvent),
    /// Audit: pipeline / reducer failure (best-effort append before `Err` returned to caller).
    ErrorOccurred(ErrorOccurredEvent),
}

/// Static kind for rule registry (`consumes` / `produces`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    MotionDetected,
    TurnOnLight,
    NotifyUser,
    LightOn,
    LightOff,
    LogUsage,
    UsageLogged,
    CommandIo,
    StateCorrectedFromObservation,
    ErrorOccurred,
}

impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Event::Fact(FactEvent::LightOn { .. }) => EventKind::LightOn,
            Event::Fact(FactEvent::LightOff { .. }) => EventKind::LightOff,
            Event::Fact(FactEvent::UsageLogged { .. }) => EventKind::UsageLogged,
            Event::Fact(FactEvent::CommandIo { .. }) => EventKind::CommandIo,
            Event::Fact(FactEvent::StateCorrectedFromObservation { .. }) => {
                EventKind::StateCorrectedFromObservation
            }
            Event::Command(CommandEvent::TurnOnLight { .. }) => EventKind::TurnOnLight,
            Event::Command(CommandEvent::NotifyUser { .. }) => EventKind::NotifyUser,
            Event::Command(CommandEvent::LogUsage { .. }) => EventKind::LogUsage,
            Event::Observation(ObservationEvent::MotionDetected { .. }) => {
                EventKind::MotionDetected
            }
            Event::ErrorOccurred(_) => EventKind::ErrorOccurred,
        }
    }

    pub fn as_fact(&self) -> Option<&FactEvent> {
        match self {
            Event::Fact(f) => Some(f),
            _ => None,
        }
    }
}

impl FactEvent {
    pub fn provenance(&self) -> Provenance {
        match self {
            FactEvent::LightOn { provenance, .. }
            | FactEvent::LightOff { provenance, .. }
            | FactEvent::UsageLogged { provenance, .. }
            | FactEvent::CommandIo { provenance, .. }
            | FactEvent::StateCorrectedFromObservation { provenance, .. } => *provenance,
        }
    }
}
