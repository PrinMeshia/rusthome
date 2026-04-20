//! Optional `sensor_display.json` in `data_dir` — libellé / pièce par capteur (hors journal).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusthome_core::State;
use serde::{Deserialize, Serialize};

pub const SENSOR_DISPLAY_FILENAME: &str = "sensor_display.json";

pub const FAMILY_TEMPERATURE: &str = "temperature";
pub const FAMILY_HUMIDITY: &str = "humidity";
pub const FAMILY_CONTACT: &str = "contact";

const MAX_SCHEMA_VERSION: u32 = 1;
const MAX_ID_LEN: usize = 256;
const MAX_LABEL_LEN: usize = 256;
const MAX_ROOM_LEN: usize = 256;

/// Per-sensor display metadata (MQTT/journal `sensor_id` remains authoritative).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SensorMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
}

/// Versioned document stored as `{data_dir}/sensor_display.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorDisplay {
    #[serde(default = "schema_version_default")]
    pub schema_version: u32,
    /// Family → sensor_id → metadata. Orphan IDs (no current reading) are kept on purpose.
    #[serde(default)]
    pub entries: BTreeMap<String, BTreeMap<String, SensorMeta>>,
}

fn schema_version_default() -> u32 {
    1
}

impl Default for SensorDisplay {
    fn default() -> Self {
        Self {
            schema_version: 1,
            entries: BTreeMap::new(),
        }
    }
}

pub fn sensor_display_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SENSOR_DISPLAY_FILENAME)
}

/// Load from disk, or default if missing. Returns I/O errors only for unreadable existing files.
pub fn load_or_default(path: &Path) -> io::Result<SensorDisplay> {
    if !path.exists() {
        return Ok(SensorDisplay::default());
    }
    let bytes = fs::read(path)?;
    let mut d: SensorDisplay = serde_json::from_slice(&bytes).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("sensor_display.json: {e}"),
        )
    })?;
    normalize_document(&mut d);
    Ok(d)
}

fn normalize_document(d: &mut SensorDisplay) {
    if d.schema_version == 0 {
        d.schema_version = 1;
    }
    for m in d.entries.values_mut() {
        for meta in m.values_mut() {
            trim_opt(&mut meta.label);
            trim_opt(&mut meta.room);
        }
    }
}

fn trim_opt(s: &mut Option<String>) {
    if let Some(ref t) = s {
        let x = t.trim();
        if x.is_empty() {
            *s = None;
        } else {
            *s = Some(x.to_string());
        }
    }
}

/// Atomic write: temp file in same directory then `rename`.
pub fn save(path: &Path, d: &SensorDisplay) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut to_save = d.clone();
    normalize_document(&mut to_save);
    let json = serde_json::to_vec_pretty(&to_save).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("serde: {e}"))
    })?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Add empty metadata entries for every sensor id present in `state` (does not remove orphans).
pub fn merge_from_state(state: &State, d: &mut SensorDisplay) {
    d.schema_version = d.schema_version.max(1).min(MAX_SCHEMA_VERSION);
    merge_family_keys(
        d,
        FAMILY_TEMPERATURE,
        state.temperature_readings().keys(),
    );
    merge_family_keys(d, FAMILY_HUMIDITY, state.humidity_readings().keys());
    merge_family_keys(d, FAMILY_CONTACT, state.contact_states().keys());
}

fn merge_family_keys<'a, I: Iterator<Item = &'a String>>(
    d: &mut SensorDisplay,
    family: &str,
    ids: I,
) {
    let map = d.entries.entry(family.to_string()).or_insert_with(BTreeMap::new);
    for id in ids {
        map.entry(id.clone()).or_insert_with(SensorMeta::default);
    }
}

/// Validate a full document before accepting a PUT.
pub fn validate_document(d: &SensorDisplay) -> Result<(), String> {
    if d.schema_version == 0 || d.schema_version > MAX_SCHEMA_VERSION {
        return Err(format!(
            "schema_version must be 1..={MAX_SCHEMA_VERSION}, got {}",
            d.schema_version
        ));
    }
    for (fam, sensors) in &d.entries {
        if fam != FAMILY_TEMPERATURE && fam != FAMILY_HUMIDITY && fam != FAMILY_CONTACT {
            return Err(format!("unknown family: {fam}"));
        }
        if sensors.len() > 10_000 {
            return Err("too many sensor entries".into());
        }
        for (id, meta) in sensors {
            if id.is_empty() || id.len() > MAX_ID_LEN {
                return Err("invalid sensor id length".into());
            }
            if let Some(ref l) = meta.label {
                if l.len() > MAX_LABEL_LEN {
                    return Err("label too long".into());
                }
            }
            if let Some(ref r) = meta.room {
                if r.len() > MAX_ROOM_LEN {
                    return Err("room too long".into());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusthome_core::State;

    fn state_from_json(v: serde_json::Value) -> State {
        serde_json::from_value(v).expect("state json")
    }

    #[test]
    fn merge_adds_keys_preserves_labels() {
        let state = state_from_json(serde_json::json!({
            "lights": {},
            "temperatures": { "a": 20000, "b": 21000 }
        }));

        let mut d = SensorDisplay::default();
        d.entries
            .entry(FAMILY_TEMPERATURE.to_string())
            .or_insert_with(BTreeMap::new)
            .insert(
                "a".into(),
                SensorMeta {
                    label: Some("Salle A".into()),
                    room: None,
                },
            );

        merge_from_state(&state, &mut d);

        let t = d.entries.get(FAMILY_TEMPERATURE).unwrap();
        assert_eq!(
            t.get("a").unwrap().label.as_deref(),
            Some("Salle A"),
            "existing label preserved"
        );
        assert!(t.contains_key("b"), "new id from state added");
        assert_eq!(t.get("b").unwrap(), &SensorMeta::default());
    }

    #[test]
    fn merge_keeps_orphan_not_in_state() {
        let state = state_from_json(serde_json::json!({
            "lights": {},
            "temperatures": { "only": 20000 }
        }));

        let mut d = SensorDisplay::default();
        d.entries
            .entry(FAMILY_TEMPERATURE.to_string())
            .or_insert_with(BTreeMap::new)
            .insert(
                "ghost".into(),
                SensorMeta {
                    label: Some("Orphelin".into()),
                    room: Some("Cave".into()),
                },
            );

        merge_from_state(&state, &mut d);

        let t = d.entries.get(FAMILY_TEMPERATURE).unwrap();
        assert!(t.contains_key("ghost"));
        assert_eq!(t.get("ghost").unwrap().label.as_deref(), Some("Orphelin"));
        assert!(t.contains_key("only"));
    }

    #[test]
    fn validate_rejects_unknown_family() {
        let mut d = SensorDisplay::default();
        d.entries.insert("pressure".into(), BTreeMap::new());
        assert!(validate_document(&d).is_err());
    }
}
