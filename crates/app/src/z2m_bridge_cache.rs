//! Shared cache for Zigbee2MQTT `bridge/info` (permit join visibility in rusthome web UI).
//!
//! Updated by the MQTT ingest loop in `rusthome serve` when subscribed to
//! `{mqtt_topic_prefix}/bridge/info`. See [docs on MQTT](https://www.zigbee2mqtt.io/guide/usage/mqtt_topics_and_messages.html).

use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::mqtt_ingest::wall_millis;

/// Last known values from a valid `bridge/info` JSON payload.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Z2mBridgeSnapshot {
    /// `permit_join` from Zigbee2MQTT, when present in the payload.
    pub permit_join: Option<bool>,
    /// Wall-clock millis when we last successfully parsed a payload (see `wall_millis` in `mqtt_ingest`).
    pub updated_ms: Option<i64>,
}

pub type Z2mBridgeCache = Arc<Mutex<Z2mBridgeSnapshot>>;

/// Topic suffix Zigbee2MQTT uses for the bridge information document.
pub const Z2M_BRIDGE_INFO_SUFFIX: &str = "bridge/info";

/// Builds `{prefix}/bridge/info` with a normalized prefix (no leading/trailing slash).
pub fn z2m_bridge_info_topic(mqtt_topic_prefix: &str) -> String {
    let p = mqtt_topic_prefix.trim_matches('/');
    format!("{p}/{Z2M_BRIDGE_INFO_SUFFIX}")
}

/// Parse `bridge/info` JSON and update `snap` on success. Returns `true` if `permit_join` was read.
pub fn apply_z2m_bridge_info_payload(snap: &mut Z2mBridgeSnapshot, payload: &[u8]) -> bool {
    let v: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if let Some(b) = v.get("permit_join").and_then(serde_json::Value::as_bool) {
        snap.permit_join = Some(b);
        snap.updated_ms = Some(wall_millis());
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_permit_join_true() {
        let mut s = Z2mBridgeSnapshot::default();
        assert!(apply_z2m_bridge_info_payload(
            &mut s,
            br#"{"permit_join":true,"commit":"abc"}"#,
        ));
        assert_eq!(s.permit_join, Some(true));
        assert!(s.updated_ms.is_some());
    }

    #[test]
    fn parses_permit_join_false() {
        let mut s = Z2mBridgeSnapshot::default();
        assert!(apply_z2m_bridge_info_payload(
            &mut s,
            br#"{"permit_join":false}"#,
        ));
        assert_eq!(s.permit_join, Some(false));
    }

    #[test]
    fn invalid_json_no_update() {
        let mut s = Z2mBridgeSnapshot {
            permit_join: Some(true),
            updated_ms: Some(1),
        };
        assert!(!apply_z2m_bridge_info_payload(&mut s, b"not json"));
        assert_eq!(s.permit_join, Some(true));
    }
}
