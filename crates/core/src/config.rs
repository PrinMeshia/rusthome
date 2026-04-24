//! Physical projection mode (plan §14.5) — shared enum; concrete snapshot TOML lives in `rusthome-app`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalProjectionMode {
    #[default]
    Simulation,
    IoAnchored,
}
