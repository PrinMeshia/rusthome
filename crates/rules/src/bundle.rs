//! Rule bundle assembly — single place for `Arc<dyn Rule>` (presets, tests, extensions).

use std::sync::Arc;

use rusthome_core::Rule;

/// Full §16 demo: R1–R5.
pub(crate) fn arc_rules_v0() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(crate::rules_impl::R1),
        Arc::new(crate::rules_impl::R2),
        Arc::new(crate::rules_impl::R3),
        Arc::new(crate::rules_impl::R4),
        Arc::new(crate::rules_impl::R5),
    ]
}

/// Home: light + IO + usage log (R1 + R3 + R4 + R5), **without** motion notify (no R2).
pub(crate) fn arc_rules_home() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(crate::rules_impl::R1),
        Arc::new(crate::rules_impl::R3),
        Arc::new(crate::rules_impl::R4),
        Arc::new(crate::rules_impl::R5),
    ]
}

/// Subset: motion → light + IO (R1 + R3).
pub(crate) fn arc_rules_minimal() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(crate::rules_impl::R1),
        Arc::new(crate::rules_impl::R3),
    ]
}
