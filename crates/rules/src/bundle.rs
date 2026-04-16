//! Rule bundle assembly — single place for `Arc<dyn Rule>` (presets, tests, extensions).

use std::sync::Arc;

use rusthome_core::{EventKind, Rule};

use crate::whitelist::ExceptionalFamilyTransition;

/// R8 + R9 whitelist: Observation → Fact (temperature and contact readings bypass the
/// default Observation → Command → Fact chain because the observation IS the data).
pub(crate) static SENSOR_WHITELIST: &[ExceptionalFamilyTransition] = &[
    ExceptionalFamilyTransition {
        rule_id: "R8",
        consumed_kind: EventKind::TemperatureReading,
        produced_kind: EventKind::TemperatureRecorded,
    },
    ExceptionalFamilyTransition {
        rule_id: "R9",
        consumed_kind: EventKind::ContactChanged,
        produced_kind: EventKind::ContactStateChanged,
    },
];

/// Full demo: R1–R5 + R7 + R8–R11 (lights + sensors + logging).
pub(crate) fn arc_rules_v0() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(crate::rules_impl::R1),
        Arc::new(crate::rules_impl::R2),
        Arc::new(crate::rules_impl::R3),
        Arc::new(crate::rules_impl::R7),
        Arc::new(crate::rules_impl::R4),
        Arc::new(crate::rules_impl::R5),
        Arc::new(crate::rules_impl::R8),
        Arc::new(crate::rules_impl::R9),
        Arc::new(crate::rules_impl::R10),
        Arc::new(crate::rules_impl::R11),
    ]
}

/// Home: lights + IO + sensors + logging (no R2 notify).
pub(crate) fn arc_rules_home() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(crate::rules_impl::R1),
        Arc::new(crate::rules_impl::R3),
        Arc::new(crate::rules_impl::R7),
        Arc::new(crate::rules_impl::R4),
        Arc::new(crate::rules_impl::R5),
        Arc::new(crate::rules_impl::R8),
        Arc::new(crate::rules_impl::R9),
        Arc::new(crate::rules_impl::R10),
        Arc::new(crate::rules_impl::R11),
    ]
}

/// Subset: motion → light + IO (R1 + R3 + R7), sensor facts (R8 + R9).
pub(crate) fn arc_rules_minimal() -> Vec<Arc<dyn Rule>> {
    vec![
        Arc::new(crate::rules_impl::R1),
        Arc::new(crate::rules_impl::R3),
        Arc::new(crate::rules_impl::R7),
        Arc::new(crate::rules_impl::R8),
        Arc::new(crate::rules_impl::R9),
    ]
}
