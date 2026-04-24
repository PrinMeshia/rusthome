//! Observation family — external signals; rules + pipeline react.

use serde::{Deserialize, Serialize};

/// Observations — external signals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum ObservationEvent {
    /// Motion in a room (V0 kernel).
    MotionDetected {
        room: String,
    },
    /// Sensor readings and contact (V0 extension).
    TemperatureReading {
        sensor_id: String,
        millidegrees_c: i32,
    },
    ContactChanged {
        sensor_id: String,
        open: bool,
    },
    HumidityReading {
        sensor_id: String,
        permille_rh: i32,
    },
}
