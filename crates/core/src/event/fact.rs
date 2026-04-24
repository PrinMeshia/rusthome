//! Fact family — the only one reduced by `apply_event` (in [`crate::reducer`]).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{CommandIoPhase, LightActuatorState, Provenance};

/// Facts — only family that flows through `apply_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum FactEvent {
    // --- V0 “kernel”: lights, usage ---
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
    // --- IO / reconciliation / audit (extended) ---
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
    /// Sensor / climate facts (V0 extension) — still deserialized for on-disk journals.
    /// Temperature reading committed to projection (millidegrees Celsius, e.g. 21500 = 21.5 °C).
    TemperatureRecorded {
        sensor_id: String,
        millidegrees_c: i32,
        provenance: Provenance,
    },
    /// Contact sensor state change (door/window; `true` = open).
    ContactStateChanged {
        sensor_id: String,
        open: bool,
        provenance: Provenance,
    },
    /// Relative humidity committed to projection (permille: 655 = 65.5 %).
    HumidityRecorded {
        sensor_id: String,
        permille_rh: i32,
        provenance: Provenance,
    },
}

impl FactEvent {
    pub fn provenance(&self) -> Provenance {
        match self {
            FactEvent::LightOn { provenance, .. }
            | FactEvent::LightOff { provenance, .. }
            | FactEvent::UsageLogged { provenance, .. }
            | FactEvent::CommandIo { provenance, .. }
            | FactEvent::StateCorrectedFromObservation { provenance, .. }
            | FactEvent::TemperatureRecorded { provenance, .. }
            | FactEvent::ContactStateChanged { provenance, .. }
            | FactEvent::HumidityRecorded { provenance, .. } => *provenance,
        }
    }
}
