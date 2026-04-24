//! Reducer and domain validation errors (plan §4.1). Journal line errors live in `rusthome_journal` (`JournalSchemaError`); orchestration in `rusthome_app` (`RunError`).

use thiserror::Error;

/// Business precondition violation — no state mutation (plan §4.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplyError {
    #[error("light already on for room {0}")]
    LightAlreadyOn(String),
    #[error("light already off for room {0}")]
    LightAlreadyOff(String),
    #[error("unknown room {0}")]
    UnknownRoom(String),
    #[error("command IO: duplicate Dispatched for key {0}")]
    IoDuplicateDispatch(String),
    #[error("command IO: terminal phase without open Dispatched for key {0}")]
    IoTerminalWithoutOpenDispatch(String),
    #[error("command IO: retry budget exhausted for key {0}")]
    IoRetryBudgetExhausted(String),
}

impl ApplyError {
    /// Stable id for `ErrorOccurred.error_type` (EPIC 4).
    pub fn stable_type_id(&self) -> &'static str {
        match self {
            ApplyError::LightAlreadyOn(_) => "apply.light_already_on",
            ApplyError::LightAlreadyOff(_) => "apply.light_already_off",
            ApplyError::UnknownRoom(_) => "apply.unknown_room",
            ApplyError::IoDuplicateDispatch(_) => "apply.io_duplicate_dispatch",
            ApplyError::IoTerminalWithoutOpenDispatch(_) => "apply.io_terminal_without_dispatch",
            ApplyError::IoRetryBudgetExhausted(_) => "apply.io_retry_budget_exhausted",
        }
    }
}
