//! Snapshot injected at process start (plan §6.12, §14.5).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalProjectionMode {
    #[default]
    Simulation,
    IoAnchored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub physical_projection_mode: PhysicalProjectionMode,
    /// Logical time delta §3: `Dispatched.logical_deadline = trigger_ts + delta` (EPIC 2).
    #[serde(default = "default_io_timeout_logical_delta")]
    pub io_timeout_logical_delta: i64,
}

fn default_io_timeout_logical_delta() -> i64 {
    60
}

impl Default for ConfigSnapshot {
    fn default() -> Self {
        Self {
            physical_projection_mode: PhysicalProjectionMode::Simulation,
            io_timeout_logical_delta: default_io_timeout_logical_delta(),
        }
    }
}
