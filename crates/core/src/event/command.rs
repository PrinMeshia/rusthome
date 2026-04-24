//! Command family — intentions; never applied directly to `State` by the reducer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Commands — intentions, never applied directly to state.
/// `command_id` required (EPIC 3) — use `rusthome_rules::deterministic_command_id` in rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum CommandEvent {
    TurnOnLight { room: String, command_id: Uuid },
    TurnOffLight { room: String, command_id: Uuid },
    NotifyUser { command_id: Uuid },
    LogUsage { item: String, command_id: Uuid },
}

impl CommandEvent {
    pub fn command_id(&self) -> Uuid {
        match self {
            CommandEvent::TurnOnLight { command_id, .. }
            | CommandEvent::TurnOffLight { command_id, .. }
            | CommandEvent::NotifyUser { command_id }
            | CommandEvent::LogUsage { command_id, .. } => *command_id,
        }
    }
}
