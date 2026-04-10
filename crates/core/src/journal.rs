//! Canonical journal line shape (plan §8.0–8.2, metadata §15).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event::Event;

pub const SCHEMA_VERSION: u32 = 3;

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
}
