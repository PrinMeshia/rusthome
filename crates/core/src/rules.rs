//! Rule trait — Option B runtime registry (plan §6.10).

use uuid::Uuid;

use crate::event::{Event, EventKind};
use crate::host_runtime_config::HostRuntimeConfig;
use crate::view::StateView;

/// Context for rule evaluation — pure inputs (plan §6.12).
pub struct RuleContext<'a> {
    pub state: &'a dyn StateView,
    pub config: &'a dyn HostRuntimeConfig,
    /// Logical timestamp of the triggering event (journal line, §3).
    pub trigger_timestamp: i64,
    /// Root causation id for this cascade (plan §15).
    pub causal_chain_id: Uuid,
    pub parent_sequence: Option<u64>,
    pub parent_event_id: Option<Uuid>,
}

/// A rule evaluates one incoming event and may emit zero or more follow-up events.
///
/// Metadata is borrowed from `self` (not `&'static`) so rules can be built at runtime
/// (config, plugins) and held behind [`std::sync::Arc`] in a registry.
pub trait Rule: Send + Sync {
    fn rule_id(&self) -> &str;
    fn priority(&self) -> i32;
    fn consumes(&self) -> &[EventKind];
    fn produces(&self) -> &[EventKind];
    /// Tags §6.14 — documentation / future policy (not used on the hot eval path).
    fn namespaces(&self) -> Vec<&str>;
    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event>;
}
