//! Top-level `Event` envelope and [`EventKind`] for the rule registry.

use serde::{Deserialize, Serialize};

use super::command::CommandEvent;
use super::error_event::ErrorOccurredEvent;
use super::fact::FactEvent;
use super::observation::ObservationEvent;

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
    TurnOffLight,
    NotifyUser,
    LightOn,
    LightOff,
    LogUsage,
    UsageLogged,
    CommandIo,
    StateCorrectedFromObservation,
    ErrorOccurred,
    TemperatureReading,
    ContactChanged,
    TemperatureRecorded,
    ContactStateChanged,
    HumidityReading,
    HumidityRecorded,
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
            Event::Fact(FactEvent::TemperatureRecorded { .. }) => EventKind::TemperatureRecorded,
            Event::Fact(FactEvent::ContactStateChanged { .. }) => EventKind::ContactStateChanged,
            Event::Fact(FactEvent::HumidityRecorded { .. }) => EventKind::HumidityRecorded,
            Event::Command(CommandEvent::TurnOnLight { .. }) => EventKind::TurnOnLight,
            Event::Command(CommandEvent::TurnOffLight { .. }) => EventKind::TurnOffLight,
            Event::Command(CommandEvent::NotifyUser { .. }) => EventKind::NotifyUser,
            Event::Command(CommandEvent::LogUsage { .. }) => EventKind::LogUsage,
            Event::Observation(ObservationEvent::MotionDetected { .. }) => {
                EventKind::MotionDetected
            }
            Event::Observation(ObservationEvent::TemperatureReading { .. }) => {
                EventKind::TemperatureReading
            }
            Event::Observation(ObservationEvent::ContactChanged { .. }) => {
                EventKind::ContactChanged
            }
            Event::Observation(ObservationEvent::HumidityReading { .. }) => {
                EventKind::HumidityReading
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
