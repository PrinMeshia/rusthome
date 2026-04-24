//! Runtime host policy read by rules (plan §14.5) — no TOML / serde; inject via [`crate::config::PhysicalProjectionMode`] and timeouts.

use crate::config::PhysicalProjectionMode;

/// What rules may read of host runtime policy: physical projection and IO clock (plan §3, §6.16).
pub trait HostRuntimeConfig {
    fn physical_projection_mode(&self) -> PhysicalProjectionMode;
    fn io_timeout_logical_delta(&self) -> i64;
}

/// Test / registry defaults — [`PhysicalProjectionMode::Simulation`], 60s logical IO timeout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DefaultHostConfig;

impl HostRuntimeConfig for DefaultHostConfig {
    fn physical_projection_mode(&self) -> PhysicalProjectionMode {
        PhysicalProjectionMode::Simulation
    }
    fn io_timeout_logical_delta(&self) -> i64 {
        60
    }
}
