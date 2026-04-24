//! Process startup configuration from `rusthome.toml` / CLI (plan §6.12, §14.5) — [`HostRuntimeConfig`] for rules.

use serde::{Deserialize, Serialize};

use rusthome_core::{HostRuntimeConfig, PhysicalProjectionMode};

/// Snapshot injected at process start: physical projection and logical IO clock (TOML + overrides).
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

impl HostRuntimeConfig for ConfigSnapshot {
    fn physical_projection_mode(&self) -> PhysicalProjectionMode {
        // Avoid method/field name clash (same ident).
        let &ConfigSnapshot {
            physical_projection_mode, ..
        } = self;
        physical_projection_mode
    }

    fn io_timeout_logical_delta(&self) -> i64 {
        let &ConfigSnapshot {
            io_timeout_logical_delta, ..
        } = self;
        io_timeout_logical_delta
    }
}
