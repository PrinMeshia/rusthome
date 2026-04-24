//! §8.3 — Canonical JSON: objects with recursively sorted keys (serde_json).

use serde::Serialize;
use serde_json::{Map, Value};

/// Serialize `v` to a single JSON line with lexicographically sorted object keys at each level.
pub fn to_canonical_line<T: Serialize>(v: &T) -> Result<String, serde_json::Error> {
    let val = serde_json::to_value(v)?;
    let sorted = sort_object_keys(val);
    let s = serde_json::to_string(&sorted)?;
    // Compact single line — serde_json::to_string already minified
    debug_assert!(!s.contains('\n'));
    Ok(s)
}

fn sort_object_keys(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut out = Map::with_capacity(map.len());
            for k in keys {
                let inner = map.get(&k).expect("key from keys()").clone();
                out.insert(k, sort_object_keys(inner));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_object_keys).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusthome_core::{Event, ObservationEvent};
    use rusthome_journal::{JournalEntry, SCHEMA_VERSION};
    use uuid::Uuid;

    #[test]
    fn roundtrip_canonical_stable() {
        let e = JournalEntry {
            schema_version: SCHEMA_VERSION,
            timestamp: 1,
            sequence: 0,
            event_id: None,
            causal_chain_id: Uuid::nil(),
            parent_sequence: None,
            parent_event_id: None,
            rule_id: None,
            correlation_id: None,
            trace_id: None,
            event: Event::Observation(ObservationEvent::MotionDetected { room: "a".into() }),
        };
        let line1 = to_canonical_line(&e).unwrap();
        let line2 = to_canonical_line(&e).unwrap();
        assert_eq!(line1, line2);
        let parsed: JournalEntry = serde_json::from_str(&line1).unwrap();
        assert_eq!(parsed.timestamp, e.timestamp);
    }
}
