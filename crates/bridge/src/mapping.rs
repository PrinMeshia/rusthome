//! Pure Zigbee2MQTT JSON → rusthome contract topics/payloads.

use serde_json::{json, Value};

use crate::config::{DeviceRule, FieldMapping};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedPublish {
    pub topic: String,
    pub payload: Vec<u8>,
}

/// Returns the friendly name segment for a device state topic, or `None` for
/// `zigbee2mqtt/bridge/...`, `.../set`, or other multi-segment paths.
pub fn extract_z2m_friendly_name(topic_prefix: &str, topic: &str) -> Option<String> {
    let prefix = topic_prefix.trim_matches('/').trim();
    let topic = topic.trim_end_matches('/');
    if prefix.is_empty() {
        return None;
    }
    let head = format!("{}/", prefix);
    let rest = topic.strip_prefix(&head)?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    Some(rest.to_string())
}

/// Map one Z2M state message to zero or more contract publishes.
pub fn translate_z2m_publish(
    topic_prefix: &str,
    topic: &str,
    payload: &[u8],
    devices: &[DeviceRule],
) -> Vec<TranslatedPublish> {
    let Some(fname) = extract_z2m_friendly_name(topic_prefix, topic) else {
        return vec![];
    };
    let Some(dev) = devices
        .iter()
        .find(|d| d.match_friendly_name == fname)
    else {
        return vec![];
    };
    let Ok(root) = serde_json::from_slice::<Value>(payload) else {
        return vec![];
    };
    let mut out = Vec::new();
    for m in &dev.mapping {
        if let Some(p) = map_one_field(&root, m) {
            out.push(p);
        }
    }
    out
}

fn map_one_field(root: &Value, m: &FieldMapping) -> Option<TranslatedPublish> {
    let v = root.get(m.json_key.trim())?;
    if v.is_null() {
        return None;
    }
    let family = m.family.to_lowercase();
    match family.as_str() {
        "temperature" => {
            let c = json_number_to_f64(v)?;
            let topic = format!("sensors/temperature/{}", m.rusthome_id);
            let payload = json!({ "celsius": c }).to_string().into_bytes();
            Some(TranslatedPublish { topic, payload })
        }
        "humidity" => {
            let pct = json_number_to_f64(v)?;
            let topic = format!("sensors/humidity/{}", m.rusthome_id);
            let payload = json!({ "humidity": pct }).to_string().into_bytes();
            Some(TranslatedPublish { topic, payload })
        }
        "contact" => {
            let open = if m.json_key == "contact" {
                let b = v.as_bool()?;
                !b
            } else {
                v.as_bool()?
            };
            let topic = format!("sensors/contact/{}", m.rusthome_id);
            let payload = json!({ "open": open }).to_string().into_bytes();
            Some(TranslatedPublish { topic, payload })
        }
        "motion" => {
            let motion = v.as_bool()?;
            if !motion {
                return None;
            }
            let topic = format!("sensors/motion/{}", m.rusthome_id);
            let room = m.rusthome_id.clone();
            let payload = json!({ "room": room }).to_string().into_bytes();
            Some(TranslatedPublish { topic, payload })
        }
        _ => None,
    }
}

fn json_number_to_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_u64().map(|u| u as f64))
        .or_else(|| v.as_i64().map(|i| i as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DeviceRule, FieldMapping};

    fn sample_devices() -> Vec<DeviceRule> {
        vec![
            DeviceRule {
                match_friendly_name: "living_multi".to_string(),
                mapping: vec![
                    FieldMapping {
                        json_key: "temperature".to_string(),
                        family: "temperature".to_string(),
                        rusthome_id: "living".to_string(),
                    },
                    FieldMapping {
                        json_key: "humidity".to_string(),
                        family: "humidity".to_string(),
                        rusthome_id: "living".to_string(),
                    },
                ],
            },
            DeviceRule {
                match_friendly_name: "front_door".to_string(),
                mapping: vec![FieldMapping {
                    json_key: "contact".to_string(),
                    family: "contact".to_string(),
                    rusthome_id: "front_door".to_string(),
                }],
            },
            DeviceRule {
                match_friendly_name: "hall_motion".to_string(),
                mapping: vec![FieldMapping {
                    json_key: "occupancy".to_string(),
                    family: "motion".to_string(),
                    rusthome_id: "hall".to_string(),
                }],
            },
        ]
    }

    #[test]
    fn skips_bridge_topics() {
        let devs = sample_devices();
        let out = translate_z2m_publish(
            "zigbee2mqtt",
            "zigbee2mqtt/bridge/devices",
            br#"{}"#,
            &devs,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn skips_unknown_friendly_name() {
        let devs = sample_devices();
        let out = translate_z2m_publish(
            "zigbee2mqtt",
            "zigbee2mqtt/unknown_device",
            br#"{"temperature":21}"#,
            &devs,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn temperature_and_humidity_from_one_payload() {
        let devs = sample_devices();
        let out = translate_z2m_publish(
            "zigbee2mqtt",
            "zigbee2mqtt/living_multi",
            br#"{"temperature":21.5,"humidity":55}"#,
            &devs,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].topic, "sensors/temperature/living");
        assert_eq!(out[1].topic, "sensors/humidity/living");
        let v0: Value = serde_json::from_slice(&out[0].payload).unwrap();
        assert_eq!(v0["celsius"], json!(21.5));
        let v1: Value = serde_json::from_slice(&out[1].payload).unwrap();
        assert_eq!(v1["humidity"], json!(55.0));
    }

    #[test]
    fn z2m_contact_true_means_closed() {
        let devs = sample_devices();
        let out = translate_z2m_publish(
            "zigbee2mqtt",
            "zigbee2mqtt/front_door",
            br#"{"contact":true}"#,
            &devs,
        );
        assert_eq!(out.len(), 1);
        let v: Value = serde_json::from_slice(&out[0].payload).unwrap();
        assert_eq!(v["open"], json!(false));
    }

    #[test]
    fn motion_only_when_occupancy_true() {
        let devs = sample_devices();
        let out = translate_z2m_publish(
            "zigbee2mqtt",
            "zigbee2mqtt/hall_motion",
            br#"{"occupancy":true}"#,
            &devs,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].topic, "sensors/motion/hall");
        let v: Value = serde_json::from_slice(&out[0].payload).unwrap();
        assert_eq!(v["room"], json!("hall"));

        let out_clear = translate_z2m_publish(
            "zigbee2mqtt",
            "zigbee2mqtt/hall_motion",
            br#"{"occupancy":false}"#,
            &devs,
        );
        assert!(out_clear.is_empty());
    }

    #[test]
    fn extract_name_ok() {
        assert_eq!(
            extract_z2m_friendly_name("zigbee2mqtt", "zigbee2mqtt/foo").as_deref(),
            Some("foo")
        );
    }
}
