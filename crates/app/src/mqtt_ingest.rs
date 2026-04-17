//! MQTT message → event dispatch and ingestion.
//!
//! Parses MQTT topic + payload to determine the event type:
//!
//! | Topic pattern | Event |
//! |---|---|
//! | `sensors/motion/{room}` | `ObservationEvent::MotionDetected` |
//! | `sensors/temperature/{sensor_id}` | `ObservationEvent::TemperatureReading` |
//! | `sensors/contact/{sensor_id}` | `ObservationEvent::ContactChanged` |
//! | `commands/light/{room}/on` | `CommandEvent::TurnOnLight` |
//! | `commands/light/{room}/off` | `CommandEvent::TurnOffLight` |
//!
//! Used by the embedded broker (`rusthome serve`) and the standalone
//! `mqtt_motion_ingest` example (backward-compatible).

use std::time::{SystemTime, UNIX_EPOCH};

use rusthome_core::{CommandEvent, ConfigSnapshot, ObservationEvent, RunError, State};
use rusthome_infra::Journal;
use rusthome_rules::Registry;
use uuid::Uuid;

use crate::{ingest_command_with_causal, ingest_observation_with_causal, RunLimits};

/// Wall-clock milliseconds since epoch.
pub fn wall_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Monotonically-increasing timestamp: max(last + 1, candidate).
pub fn next_ts(last_ts: &mut i64, candidate: i64) -> i64 {
    let n = (*last_ts + 1).max(candidate);
    *last_ts = n;
    n
}

/// Try to extract an entity name from an MQTT payload.
///
/// 1. If the payload is JSON with a `"room"` or `"sensor_id"` key, use that.
/// 2. If the payload is a short plain string (no braces), use it directly.
/// 3. Fall back to the last non-wildcard segment of the topic.
pub fn entity_from_payload_and_topic(
    payload: &[u8],
    topic: &str,
    json_key: &str,
) -> Result<String, String> {
    let s = std::str::from_utf8(payload).map_err(|_| "payload is not UTF-8".to_string())?;
    let s = s.trim();
    if s.is_empty() {
        return fallback_from_topic(topic);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(r) = v.get(json_key).and_then(|x| x.as_str()) {
            return Ok(r.to_string());
        }
    }
    if !s.contains('{') && s.len() < 256 {
        return Ok(s.to_string());
    }
    fallback_from_topic(topic)
}

fn fallback_from_topic(topic: &str) -> Result<String, String> {
    topic
        .rsplit('/')
        .find(|x| !x.is_empty() && *x != "#" && *x != "+")
        .map(String::from)
        .ok_or_else(|| "could not extract entity from payload or topic".into())
}

/// Try to extract an optional `"ts"` field from a JSON payload.
pub fn optional_ts_from_payload(payload: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(payload).ok()?.trim();
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    v.get("ts")?.as_i64()
}

/// Parse the second segment of a topic to determine the sensor category.
///
/// Returns `("motion", "hall")` for `sensors/motion/hall`.
fn parse_topic_segments(topic: &str) -> Option<(&str, &str)> {
    let stripped = topic.strip_prefix("sensors/").unwrap_or(topic);
    let (category, rest) = stripped.split_once('/')?;
    let entity = rest.split('/').next().filter(|s| !s.is_empty())?;
    Some((category, entity))
}

/// Classify an MQTT publish into an `ObservationEvent`.
///
/// Returns `None` if the topic doesn't match any known pattern.
pub fn observation_from_mqtt(
    topic: &str,
    payload: &[u8],
) -> Result<Option<ObservationEvent>, String> {
    let (category, topic_entity) = match parse_topic_segments(topic) {
        Some(pair) => pair,
        None => return Ok(None),
    };

    match category {
        "motion" => {
            let room = entity_from_payload_and_topic(payload, topic, "room")
                .unwrap_or_else(|_| topic_entity.to_string());
            Ok(Some(ObservationEvent::MotionDetected { room }))
        }
        "temperature" => {
            let sensor_id = entity_from_payload_and_topic(payload, topic, "sensor_id")
                .unwrap_or_else(|_| topic_entity.to_string());
            let millidegrees_c = parse_temperature(payload)?;
            Ok(Some(ObservationEvent::TemperatureReading {
                sensor_id,
                millidegrees_c,
            }))
        }
        "contact" => {
            let sensor_id = entity_from_payload_and_topic(payload, topic, "sensor_id")
                .unwrap_or_else(|_| topic_entity.to_string());
            let open = parse_contact(payload)?;
            Ok(Some(ObservationEvent::ContactChanged { sensor_id, open }))
        }
        _ => Ok(None),
    }
}

/// Classify an MQTT publish into a `CommandEvent`.
///
/// Returns `None` if the topic doesn't match any known command pattern.
pub fn command_from_mqtt(
    topic: &str,
    _payload: &[u8],
) -> Result<Option<CommandEvent>, String> {
    let rest = match topic.strip_prefix("commands/") {
        Some(r) => r,
        None => return Ok(None),
    };

    let segments: Vec<&str> = rest.splitn(3, '/').collect();
    match segments.as_slice() {
        ["light", room, "on"] if !room.is_empty() => Ok(Some(CommandEvent::TurnOnLight {
            room: (*room).to_string(),
            command_id: Uuid::new_v4(),
        })),
        ["light", room, "off"] if !room.is_empty() => Ok(Some(CommandEvent::TurnOffLight {
            room: (*room).to_string(),
            command_id: Uuid::new_v4(),
        })),
        ["light", ..] => Err(format!("malformed command topic: {topic}")),
        _ => Ok(None),
    }
}

fn parse_temperature(payload: &[u8]) -> Result<i32, String> {
    let s = std::str::from_utf8(payload).map_err(|_| "payload is not UTF-8")?;
    let s = s.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(mc) = v.get("millidegrees_c").and_then(|x| x.as_i64()) {
            return Ok(mc as i32);
        }
        if let Some(c) = v.get("celsius").and_then(|x| x.as_f64()) {
            return Ok((c * 1000.0) as i32);
        }
        if let Some(mc) = v.get("value").and_then(|x| x.as_i64()) {
            return Ok(mc as i32);
        }
    }
    s.parse::<i32>()
        .map_err(|_| format!("cannot parse temperature from payload: {s}"))
}

fn parse_contact(payload: &[u8]) -> Result<bool, String> {
    let s = std::str::from_utf8(payload).map_err(|_| "payload is not UTF-8")?;
    let s = s.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(b) = v.get("open").and_then(|x| x.as_bool()) {
            return Ok(b);
        }
        if let Some(b) = v.get("contact").and_then(|x| x.as_bool()) {
            return Ok(!b); // Zigbee2MQTT: contact=true means closed
        }
    }
    match s.to_ascii_lowercase().as_str() {
        "open" | "true" | "1" => Ok(true),
        "closed" | "close" | "false" | "0" => Ok(false),
        _ => Err(format!("cannot parse contact state from payload: {s}")),
    }
}

/// Ingest a single MQTT publish into the journal.
///
/// Returns `Ok(Some(description))` on success, `Ok(None)` if the topic is
/// unrecognised (silently skipped), or `Err` on pipeline failure.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_mqtt_publish(
    topic: &str,
    payload: &[u8],
    journal: &mut Journal,
    state: &mut State,
    registry: &Registry,
    config: &ConfigSnapshot,
    limits: RunLimits,
    last_ts: &mut i64,
) -> Result<Option<String>, DispatchError> {
    match observation_from_mqtt(topic, payload) {
        Ok(Some(obs)) => {
            let candidate = optional_ts_from_payload(payload).unwrap_or_else(wall_millis);
            let ts = next_ts(last_ts, candidate);
            let causal = Uuid::new_v4();
            let desc = format!("{obs:?}");
            ingest_observation_with_causal(journal, state, registry, config, ts, obs, causal, limits)?;
            return Ok(Some(desc));
        }
        Err(e) => return Err(DispatchError::Parse(e)),
        Ok(None) => {}
    }

    match command_from_mqtt(topic, payload) {
        Ok(Some(cmd)) => {
            let ts = next_ts(last_ts, wall_millis());
            let causal = Uuid::new_v4();
            let desc = format!("{cmd:?}");
            ingest_command_with_causal(journal, state, registry, config, ts, cmd, causal, limits)?;
            Ok(Some(desc))
        }
        Err(e) => Err(DispatchError::Parse(e)),
        Ok(None) => Ok(None),
    }
}

/// Errors from [`dispatch_mqtt_publish`].
#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("ingest error: {0}")]
    Run(#[from] RunError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_topic_parses() {
        let obs = observation_from_mqtt("sensors/motion/hall", b"hall").unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::MotionDetected { room }) if room == "hall"
        ));
    }

    #[test]
    fn motion_json_payload() {
        let obs =
            observation_from_mqtt("sensors/motion/x", br#"{"room":"kitchen"}"#).unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::MotionDetected { room }) if room == "kitchen"
        ));
    }

    #[test]
    fn temperature_millidegrees() {
        let obs = observation_from_mqtt(
            "sensors/temperature/outdoor",
            br#"{"millidegrees_c": -5300}"#,
        )
        .unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::TemperatureReading { sensor_id, millidegrees_c })
                if sensor_id == "outdoor" && millidegrees_c == -5300
        ));
    }

    #[test]
    fn temperature_celsius_float() {
        let obs = observation_from_mqtt(
            "sensors/temperature/living",
            br#"{"celsius": 21.5}"#,
        )
        .unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::TemperatureReading { millidegrees_c, .. })
                if millidegrees_c == 21500
        ));
    }

    #[test]
    fn temperature_raw_integer() {
        let obs =
            observation_from_mqtt("sensors/temperature/attic", b"19200").unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::TemperatureReading { millidegrees_c, .. })
                if millidegrees_c == 19200
        ));
    }

    #[test]
    fn contact_open_json() {
        let obs =
            observation_from_mqtt("sensors/contact/front-door", br#"{"open": true}"#).unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::ContactChanged { open: true, .. })
        ));
    }

    #[test]
    fn contact_closed_string() {
        let obs =
            observation_from_mqtt("sensors/contact/window", b"closed").unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::ContactChanged { open: false, .. })
        ));
    }

    #[test]
    fn contact_zigbee2mqtt_convention() {
        let obs =
            observation_from_mqtt("sensors/contact/gate", br#"{"contact": true}"#).unwrap();
        assert!(matches!(
            obs,
            Some(ObservationEvent::ContactChanged { open: false, .. })
        ));
    }

    #[test]
    fn unknown_category_returns_none() {
        let obs = observation_from_mqtt("sensors/humidity/bath", b"80").unwrap();
        assert!(obs.is_none());
    }

    #[test]
    fn non_sensor_topic_returns_none() {
        let obs = observation_from_mqtt("other/topic", b"data").unwrap();
        assert!(obs.is_none());
    }

    #[test]
    fn command_light_on() {
        let cmd = command_from_mqtt("commands/light/hall/on", b"").unwrap();
        assert!(matches!(
            cmd,
            Some(CommandEvent::TurnOnLight { room, .. }) if room == "hall"
        ));
    }

    #[test]
    fn command_light_off() {
        let cmd = command_from_mqtt("commands/light/kitchen/off", b"").unwrap();
        assert!(matches!(
            cmd,
            Some(CommandEvent::TurnOffLight { room, .. }) if room == "kitchen"
        ));
    }

    #[test]
    fn command_unknown_returns_none() {
        let cmd = command_from_mqtt("commands/thermostat/living/set", b"").unwrap();
        assert!(cmd.is_none());
    }

    #[test]
    fn command_malformed_light_topic() {
        let result = command_from_mqtt("commands/light/", b"");
        assert!(result.is_err());
    }

    #[test]
    fn non_command_topic_returns_none() {
        let cmd = command_from_mqtt("sensors/motion/hall", b"").unwrap();
        assert!(cmd.is_none());
    }
}
