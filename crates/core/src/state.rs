//! Projection state — `BTreeMap` for deterministic iteration (plan §6.12).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::event::Provenance;
use crate::view::StateView;

/// `CommandIo` tracking by key `cmd:{uuid}` or `room:{name}` (EPIC 2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandIoTracker {
    pub awaiting_terminal: bool,
    pub timeouts_seen: u8,
}

/// Domain projection. Mutations only via `crate::reducer::apply_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct State {
    /// Room id -> light is on. Mutate only via `apply_event`.
    pub(crate) lights: BTreeMap<String, bool>,
    /// Provenance of the last light fact applied for this room (Observed / Derived reconciliation).
    #[serde(default)]
    pub(crate) light_last_provenance: BTreeMap<String, Provenance>,
    #[serde(default)]
    pub(crate) command_io_trackers: BTreeMap<String, CommandIoTracker>,
    /// Last log usage item (demo). Mutate only via `apply_event`.
    pub(crate) last_log: Option<String>,
    /// Sensor id → last temperature in millidegrees Celsius.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) temperatures: BTreeMap<String, i32>,
    /// Sensor id → contact is open (true = open, false = closed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) contacts: BTreeMap<String, bool>,
}

impl StateView for State {
    fn light_on(&self, room: &str) -> bool {
        *self.lights.get(room).unwrap_or(&false)
    }

    fn last_log_item(&self) -> Option<&str> {
        self.last_log.as_deref()
    }

    fn temperature(&self, sensor_id: &str) -> Option<i32> {
        self.temperatures.get(sensor_id).copied()
    }

    fn contact_open(&self, sensor_id: &str) -> Option<bool> {
        self.contacts.get(sensor_id).copied()
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Last known provenance for light state in `room` (reconciliation).
    pub fn light_last_provenance(&self, room: &str) -> Option<Provenance> {
        self.light_last_provenance.get(room).copied()
    }

    /// Rooms in deterministic order with on/off and last provenance — for dashboards / HTTP APIs.
    pub fn light_room_rows(&self) -> Vec<(String, bool, Option<Provenance>)> {
        self.lights
            .iter()
            .map(|(room, on)| {
                (
                    room.clone(),
                    *on,
                    self.light_last_provenance.get(room).copied(),
                )
            })
            .collect()
    }

    /// All temperature readings (sensor_id → millidegrees Celsius) in deterministic order.
    pub fn temperature_readings(&self) -> &BTreeMap<String, i32> {
        &self.temperatures
    }

    /// All contact sensor states (sensor_id → open) in deterministic order.
    pub fn contact_states(&self) -> &BTreeMap<String, bool> {
        &self.contacts
    }
}
