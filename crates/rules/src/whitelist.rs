//! §6.17 — exceptional family transitions (audited, versioned).

use rusthome_core::EventKind;

use crate::family_transitions::{kind_family, transition_allowed};

/// Explicit exception: rule `rule_id` may produce `produced_kind` while consuming `consumed_kind`
/// even if the default matrix forbids the corresponding **family** pair.
#[derive(Debug, Clone, Copy)]
pub struct ExceptionalFamilyTransition {
    pub rule_id: &'static str,
    pub consumed_kind: EventKind,
    pub produced_kind: EventKind,
}

impl ExceptionalFamilyTransition {
    pub fn matches(self, rule_id: &str, consumed: EventKind, produced: EventKind) -> bool {
        self.rule_id == rule_id && self.consumed_kind == consumed && self.produced_kind == produced
    }

    /// True if this whitelist entry is redundant (the default family matrix already allows the transition).
    pub fn is_redundant(self) -> bool {
        let from = kind_family(self.consumed_kind);
        let to = kind_family(self.produced_kind);
        transition_allowed(from, to)
    }
}
