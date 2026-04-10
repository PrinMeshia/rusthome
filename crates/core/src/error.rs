//! Typed errors for reducer and pipeline (plan §4.1, §6.6).

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

/// Technical / cascade limits (plan §6.6).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RunError {
    #[error("cascade limit: max events per run exceeded ({current} > {max})")]
    MaxEventsPerRun { current: u64, max: u64 },
    #[error("cascade limit: max events generated per root exceeded ({current} > {max})")]
    MaxEventsGeneratedPerRoot { current: u64, max: u64 },
    #[error("run wall-clock budget exceeded ({elapsed_ms} ms > {max_ms} ms)")]
    RunTimeBudgetExceeded { elapsed_ms: u128, max_ms: u64 },
    #[error("pending FIFO capacity exceeded ({pending} > {max})")]
    QueueCapacityExceeded { pending: usize, max: usize },
    #[error("apply error: {0}")]
    Apply(#[from] ApplyError),
    #[error("journal error: {0}")]
    Journal(String),
    #[error(
        "IoAnchored mode: derived actuator fact ({0}) is not allowed without IO path (plan §14.5)"
    )]
    IoAnchoredDerivedActuator(String),
}

impl RunError {
    /// Stable id for `ErrorOccurred.error_type` (EPIC 4).
    pub fn stable_type_id(&self) -> String {
        match self {
            RunError::Apply(a) => a.stable_type_id().to_string(),
            RunError::MaxEventsPerRun { .. } => "run.max_events_per_run".into(),
            RunError::MaxEventsGeneratedPerRoot { .. } => {
                "run.max_events_generated_per_root".into()
            }
            RunError::RunTimeBudgetExceeded { .. } => "run.time_budget_exceeded".into(),
            RunError::QueueCapacityExceeded { .. } => "run.queue_capacity_exceeded".into(),
            RunError::Journal(_) => "run.journal".into(),
            RunError::IoAnchoredDerivedActuator(_) => "run.io_anchored_derived_actuator".into(),
        }
    }

    pub fn max_events_per_run(current: u64, max: u64) -> Self {
        Self::MaxEventsPerRun { current, max }
    }

    pub fn max_events_generated_per_root(current: u64, max: u64) -> Self {
        Self::MaxEventsGeneratedPerRoot { current, max }
    }

    pub fn run_time_budget(elapsed_ms: u128, max_ms: u64) -> Self {
        Self::RunTimeBudgetExceeded { elapsed_ms, max_ms }
    }

    pub fn queue_capacity(pending: usize, max: usize) -> Self {
        Self::QueueCapacityExceeded { pending, max }
    }

    pub fn journal(msg: impl Into<String>) -> Self {
        Self::Journal(msg.into())
    }

    pub fn io_anchored_derived_actuator(detail: impl Into<String>) -> Self {
        Self::IoAnchoredDerivedActuator(detail.into())
    }
}
