//! Read-only API for rules (plan §4.4).

/// Minimal surface passed to rule evaluation — not full `State`.
pub trait StateView: Send + Sync {
    fn light_on(&self, room: &str) -> bool;
    fn last_log_item(&self) -> Option<&str>;
    /// Last recorded temperature in millidegrees Celsius, or `None` if never seen.
    fn temperature(&self, sensor_id: &str) -> Option<i32>;
    /// Last known contact state (`true` = open), or `None` if never seen.
    fn contact_open(&self, sensor_id: &str) -> Option<bool>;
}
