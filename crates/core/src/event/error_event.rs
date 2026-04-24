//! Persisted runtime errors (EPIC 4) — no-op for `replay_state` / `apply_event`.

use serde::{Deserialize, Serialize};

/// Persisted runtime error (EPIC 4) — no-op for `replay_state` / `apply_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorOccurredEvent {
    pub error_type: String,
    pub context: String,
}
