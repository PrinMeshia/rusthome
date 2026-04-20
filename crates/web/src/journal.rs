//! Journal tail loading for the dashboard and `/api/journal`.

use std::path::Path;

use rusthome_core::{
    CommandEvent, Event, EventKind, FactEvent, ObservationEvent,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct JournalQuery {
    #[serde(default = "default_limit")]
    pub(crate) limit: usize,
}

pub(crate) fn default_limit() -> usize {
    40
}

#[derive(Serialize)]
pub(crate) struct JournalLineDto {
    pub(crate) sequence: u64,
    pub(crate) timestamp: i64,
    pub(crate) kind: EventKind,
    pub(crate) detail: String,
    pub(crate) family: &'static str,
}

fn event_family(event: &Event) -> &'static str {
    match event {
        Event::Fact(_) => "fact",
        Event::Command(_) => "command",
        Event::Observation(_) => "observation",
        Event::ErrorOccurred(_) => "error",
    }
}

fn event_detail(event: &Event) -> String {
    match event {
        Event::Observation(ObservationEvent::MotionDetected { room }) => {
            format!("motion: {room}")
        }
        Event::Observation(ObservationEvent::TemperatureReading {
            sensor_id,
            millidegrees_c,
        }) => {
            let celsius = *millidegrees_c as f64 / 1000.0;
            format!("temp: {sensor_id} {celsius:.1}\u{00B0}C")
        }
        Event::Observation(ObservationEvent::ContactChanged { sensor_id, open }) => {
            let state = if *open { "open" } else { "closed" };
            format!("contact: {sensor_id} {state}")
        }
        Event::Observation(ObservationEvent::HumidityReading {
            sensor_id,
            permille_rh,
        }) => {
            let pct = *permille_rh as f64 / 10.0;
            format!("humidity: {sensor_id} {pct:.1}%")
        }
        Event::Fact(FactEvent::LightOn { room, .. }) => format!("light: {room} on"),
        Event::Fact(FactEvent::LightOff { room, .. }) => format!("light: {room} off"),
        Event::Fact(FactEvent::UsageLogged { item, .. }) => format!("logged: {item}"),
        Event::Fact(FactEvent::CommandIo { room, phase, .. }) => {
            let target = room.as_deref().unwrap_or("?");
            format!("io: {target} {phase:?}")
        }
        Event::Fact(FactEvent::StateCorrectedFromObservation {
            entity_id,
            observed,
            ..
        }) => format!("corrected: {entity_id} \u{2192} {observed:?}"),
        Event::Fact(FactEvent::TemperatureRecorded {
            sensor_id,
            millidegrees_c,
            ..
        }) => {
            let celsius = *millidegrees_c as f64 / 1000.0;
            format!("recorded: {sensor_id} {celsius:.1}\u{00B0}C")
        }
        Event::Fact(FactEvent::ContactStateChanged {
            sensor_id, open, ..
        }) => {
            let state = if *open { "open" } else { "closed" };
            format!("recorded: {sensor_id} {state}")
        }
        Event::Fact(FactEvent::HumidityRecorded {
            sensor_id,
            permille_rh,
            ..
        }) => {
            let pct = *permille_rh as f64 / 10.0;
            format!("recorded: {sensor_id} {pct:.1}% RH")
        }
        Event::Command(CommandEvent::TurnOnLight { room, .. }) => {
            format!("cmd: turn-on {room}")
        }
        Event::Command(CommandEvent::TurnOffLight { room, .. }) => {
            format!("cmd: turn-off {room}")
        }
        Event::Command(CommandEvent::NotifyUser { .. }) => "cmd: notify-user".to_string(),
        Event::Command(CommandEvent::LogUsage { item, .. }) => {
            format!("cmd: log {item}")
        }
        Event::ErrorOccurred(err) => {
            format!("error: {} — {}", err.error_type, err.context)
        }
    }
}

pub(crate) fn journal_tail_dtos(path: &Path, limit: usize) -> Result<Vec<JournalLineDto>, String> {
    let entries = rusthome_infra::load_and_sort(path).map_err(|e| e.to_string())?;
    let lim = limit.clamp(1, 500);
    let tail = if entries.len() > lim {
        let start = entries.len() - lim;
        let mut v = entries;
        v.split_off(start)
    } else {
        entries
    };
    Ok(tail
        .into_iter()
        .map(|e| {
            let detail = event_detail(&e.event);
            let family = event_family(&e.event);
            JournalLineDto {
                sequence: e.sequence,
                timestamp: e.timestamp,
                kind: e.event.kind(),
                detail,
                family,
            }
        })
        .collect())
}
