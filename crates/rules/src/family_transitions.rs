//! §6.17 — allowed transitions between event families (design-time matrix).

use rusthome_core::EventKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Observation,
    Command,
    Fact,
    /// `ErrorOccurred` line (EPIC 4) — outside V0 rule family transition matrix.
    Error,
}

pub fn kind_family(k: EventKind) -> Family {
    match k {
        EventKind::MotionDetected
        | EventKind::TemperatureReading
        | EventKind::ContactChanged => Family::Observation,
        EventKind::TurnOnLight | EventKind::TurnOffLight | EventKind::NotifyUser | EventKind::LogUsage => {
            Family::Command
        }
        EventKind::LightOn
        | EventKind::LightOff
        | EventKind::UsageLogged
        | EventKind::CommandIo
        | EventKind::StateCorrectedFromObservation
        | EventKind::TemperatureRecorded
        | EventKind::ContactStateChanged => Family::Fact,
        EventKind::ErrorOccurred => Family::Error,
    }
}

/// Default V0 policy from plan §6.17.
pub fn transition_allowed(from: Family, to: Family) -> bool {
    if matches!(from, Family::Error) || matches!(to, Family::Error) {
        return false;
    }
    matches!(
        (from, to),
        (Family::Observation, Family::Command)
            | (Family::Command, Family::Fact)
            | (Family::Command, Family::Command)
            | (Family::Fact, Family::Command)
    )
}
