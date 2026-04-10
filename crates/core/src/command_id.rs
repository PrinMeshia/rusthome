//! EPIC 3 — deterministic `command_id` (same journal input → same id, stable replay).

use uuid::Uuid;

/// Namespace for `Uuid::new_v5` (do not change without schema migration).
pub const COMMAND_ID_NAMESPACE: Uuid = Uuid::from_u128(0x6d3b_e10c_4f5a_6e7b_8c9d_0e1f_2a3b_4c5d);

/// Stable command id: derived from rule, trigger, and causal context.
/// Do not use `Uuid::new_v4()` for persisted commands.
pub fn deterministic_command_id(
    rule_id: &str,
    command_kind: &str,
    parent_sequence: Option<u64>,
    causal_chain_id: Uuid,
    payload: &str,
) -> Uuid {
    let ps = parent_sequence.map_or_else(|| "none".to_string(), |s| s.to_string());
    let name = format!("{rule_id}\x1f{command_kind}\x1f{ps}\x1f{causal_chain_id}\x1f{payload}");
    Uuid::new_v5(&COMMAND_ID_NAMESPACE, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn deterministic_same_inputs_same_uuid() {
        let causal = Uuid::nil();
        let a = deterministic_command_id("R1", "turn_on", Some(3), causal, "hall");
        let b = deterministic_command_id("R1", "turn_on", Some(3), causal, "hall");
        assert_eq!(a, b);
    }

    #[test]
    fn different_payload_differs() {
        let causal = Uuid::nil();
        let a = deterministic_command_id("R1", "turn_on", Some(3), causal, "a");
        let b = deterministic_command_id("R1", "turn_on", Some(3), causal, "b");
        assert_ne!(a, b);
    }
}
