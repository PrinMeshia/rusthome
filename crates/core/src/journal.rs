//! Canonical journal line shape (plan §8.0–8.2, metadata §15).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JournalSchemaError;
use crate::event::Event;

/// Current schema written by this binary on append (plan §8.2).
pub const SCHEMA_VERSION: u32 = 4;

/// Lowest `schema_version` accepted on load (inclusive). Documented in repo `docs/rules-changelog.md`: 2 = `command_id` + dedup; 3 adds `ErrorOccurred`.
pub const MIN_SUPPORTED_JOURNAL_SCHEMA: u32 = 2;

/// Whether a persisted line's `schema_version` is supported (inclusive range).
#[inline]
pub fn journal_schema_supported(version: u32) -> bool {
    (MIN_SUPPORTED_JOURNAL_SCHEMA..=SCHEMA_VERSION).contains(&version)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub schema_version: u32,
    pub timestamp: i64,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
    pub causal_chain_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_event_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<Uuid>,
    #[serde(flatten)]
    pub event: Event,
}

impl JournalEntry {
    pub fn sort_key(&self) -> (i64, u64) {
        (self.timestamp, self.sequence)
    }

    /// Reject lines from unsupported journal eras before reducer / rules run.
    pub fn validate_supported_schema(&self) -> Result<(), JournalSchemaError> {
        if journal_schema_supported(self.schema_version) {
            Ok(())
        } else {
            Err(JournalSchemaError::UnsupportedVersion {
                found: self.schema_version,
                min: MIN_SUPPORTED_JOURNAL_SCHEMA,
                max: SCHEMA_VERSION,
            })
        }
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn supported_range_matches_constants() {
        assert!(!journal_schema_supported(0));
        assert!(!journal_schema_supported(1));
        assert!(journal_schema_supported(2));
        assert!(journal_schema_supported(3));
        assert!(journal_schema_supported(4));
        assert!(!journal_schema_supported(5));
    }
}
