//! Small shared types for the event families.
//!
//! Grouped for clarity; the journal still serializes a single flat `Event` envelope.

use serde::{Deserialize, Serialize};

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
