//! Rule evaluation trace — plan §15 (matched / not matched). Serialized by CLI `--trace-file`.

use serde::{Deserialize, Serialize};

use rusthome_core::EventKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEvaluationRecord {
    pub trigger_sequence: u64,
    pub trigger_kind: EventKind,
    pub rule_id: String,
    pub matched: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
