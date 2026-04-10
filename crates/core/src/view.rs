//! Read-only API for rules (plan §4.4).

/// Minimal surface passed to rule evaluation — not full `State`.
pub trait StateView: Send + Sync {
    fn light_on(&self, room: &str) -> bool;
    fn last_log_item(&self) -> Option<&str>;
}
