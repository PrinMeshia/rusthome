//! Boot-time registry — cycle check on event-type graph (plan §6.13).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rusthome_core::{
    CommandEvent, CommandIoPhase, ConfigSnapshot, Event, EventKind, FactEvent, LightActuatorState,
    ObservationEvent, PhysicalProjectionMode, Provenance, Rule, RuleContext, State,
};
use uuid::Uuid;

use crate::family_transitions::{kind_family, transition_allowed, Family};
use crate::whitelist::ExceptionalFamilyTransition;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("rule {rule_id} emits {kind:?} not declared in produces")]
    EmissionNotDeclared { rule_id: String, kind: EventKind },
    #[error("cycle detected in rule dependency graph (event kinds)")]
    CycleDetected,
    #[error("rule {rule_id} consumes {count} event types; max is {max} (plan §6.15)")]
    TooManyConsumed {
        rule_id: String,
        count: usize,
        max: usize,
    },
    #[error(
        "rule {rule_id} §6.17 transition forbidden: {from:?} -> {to:?} (consumed {ck:?} produces {pk:?})"
    )]
    FamilyTransitionForbidden {
        rule_id: String,
        from: Family,
        to: Family,
        ck: EventKind,
        pk: EventKind,
    },
    #[error(
        "whitelist §6.17 entry for rule {rule_id} is redundant (already allowed by default matrix)"
    )]
    WhitelistRedundant { rule_id: String },
}

pub struct Registry {
    rules: Vec<Arc<dyn Rule>>,
    /// Family transitions allowed outside the default matrix (plan §6.17).
    pub family_transition_whitelist: &'static [ExceptionalFamilyTransition],
}

/// Plan §6.15 — fan-in per rule.
pub const MAX_CONSUMED_EVENT_TYPES_PER_RULE: usize = 3;

impl Registry {
    /// Explicit registry — extension point for user rules / dynamic loading.
    pub fn from_rules(
        rules: Vec<Arc<dyn Rule>>,
        family_transition_whitelist: &'static [ExceptionalFamilyTransition],
    ) -> Self {
        Self {
            rules,
            family_transition_whitelist,
        }
    }

    pub fn v0_default() -> Self {
        Self::from_rules(crate::bundle::arc_rules_v0(), crate::bundle::SENSOR_WHITELIST)
    }

    /// Lights + IO + sensors + logging, **without** `NotifyUser` (R2).
    /// Default snapshot digest: `rules-home` ([`RulesPreset::Home`](crate::preset::RulesPreset)).
    pub fn home_default() -> Self {
        Self::from_rules(crate::bundle::arc_rules_home(), crate::bundle::SENSOR_WHITELIST)
    }

    /// Subset: motion → light + IO (R1 + R3 + R7), sensor facts (R8 + R9).
    pub fn minimal_default() -> Self {
        Self::from_rules(crate::bundle::arc_rules_minimal(), crate::bundle::SENSOR_WHITELIST)
    }

    pub fn rules(&self) -> &[Arc<dyn Rule>] {
        &self.rules
    }

    /// Build edges (consumed kind → produced kind) per rule; fail if cycle.
    pub fn validate_boot(&self) -> Result<(), RegistryError> {
        for w in self.family_transition_whitelist {
            if w.is_redundant() {
                return Err(RegistryError::WhitelistRedundant {
                    rule_id: w.rule_id.to_string(),
                });
            }
        }

        for r in &self.rules {
            let n = r.consumes().len();
            if n > MAX_CONSUMED_EVENT_TYPES_PER_RULE {
                return Err(RegistryError::TooManyConsumed {
                    rule_id: r.rule_id().to_string(),
                    count: n,
                    max: MAX_CONSUMED_EVENT_TYPES_PER_RULE,
                });
            }
        }

        let mut adj: HashMap<EventKind, Vec<EventKind>> = HashMap::new();
        for r in &self.rules {
            for &c in r.consumes() {
                for &p in r.produces() {
                    adj.entry(c).or_default().push(p);
                }
            }
        }
        let mut state: HashMap<EventKind, u8> = HashMap::new();
        let nodes: Vec<EventKind> = {
            let mut s: HashSet<EventKind> = HashSet::new();
            for (k, vs) in &adj {
                s.insert(*k);
                for v in vs {
                    s.insert(*v);
                }
            }
            s.into_iter().collect()
        };
        for k in nodes {
            if dfs_cycle(k, &adj, &mut state) {
                return Err(RegistryError::CycleDetected);
            }
        }

        for r in &self.rules {
            for &ck in r.consumes() {
                for &pk in r.produces() {
                    let from = kind_family(ck);
                    let to = kind_family(pk);
                    if !transition_allowed(from, to) {
                        let ok = self
                            .family_transition_whitelist
                            .iter()
                            .any(|w| w.matches(r.rule_id(), ck, pk));
                        if !ok {
                            return Err(RegistryError::FamilyTransitionForbidden {
                                rule_id: r.rule_id().to_string(),
                                from,
                                to,
                                ck,
                                pk,
                            });
                        }
                    }
                }
            }
        }

        self.validate_emissions_match_produces()?;

        Ok(())
    }

    /// Dry-run `eval` with canonical samples — §6.12.1 / §6.10 enforcement.
    fn validate_emissions_match_produces(&self) -> Result<(), RegistryError> {
        let state = State::new();
        let config = ConfigSnapshot {
            physical_projection_mode: PhysicalProjectionMode::Simulation,
            ..Default::default()
        };
        let ctx = RuleContext {
            state: &state,
            config: &config,
            trigger_timestamp: 0,
            causal_chain_id: Uuid::nil(),
            parent_sequence: None,
            parent_event_id: None,
        };
        for r in &self.rules {
            for &ck in r.consumes() {
                let Some(sample) = sample_event(ck) else {
                    continue;
                };
                for out in r.eval(&sample, &ctx) {
                    let k = out.kind();
                    if !r.produces().contains(&k) {
                        return Err(RegistryError::EmissionNotDeclared {
                            rule_id: r.rule_id().to_string(),
                            kind: k,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

fn sample_event(kind: EventKind) -> Option<Event> {
    match kind {
        EventKind::ErrorOccurred => None,
        EventKind::MotionDetected => Some(Event::Observation(ObservationEvent::MotionDetected {
            room: "_".into(),
        })),
        EventKind::TurnOnLight => Some(Event::Command(CommandEvent::TurnOnLight {
            room: "_".into(),
            command_id: Uuid::from_u128(0x0001),
        })),
        EventKind::TurnOffLight => Some(Event::Command(CommandEvent::TurnOffLight {
            room: "_".into(),
            command_id: Uuid::from_u128(0x0001_0002),
        })),
        EventKind::NotifyUser => Some(Event::Command(CommandEvent::NotifyUser {
            command_id: Uuid::from_u128(0x0002),
        })),
        EventKind::LogUsage => Some(Event::Command(CommandEvent::LogUsage {
            item: "_".into(),
            command_id: Uuid::from_u128(0x0003),
        })),
        EventKind::LightOn => Some(Event::Fact(FactEvent::LightOn {
            room: "_".into(),
            provenance: Provenance::Derived,
        })),
        EventKind::LightOff => Some(Event::Fact(FactEvent::LightOff {
            room: "_".into(),
            provenance: Provenance::Derived,
        })),
        EventKind::UsageLogged => Some(Event::Fact(FactEvent::UsageLogged {
            item: "_".into(),
            provenance: Provenance::Derived,
        })),
        EventKind::CommandIo => Some(Event::Fact(FactEvent::CommandIo {
            command_id: None,
            room: None,
            phase: CommandIoPhase::Dispatched {
                logical_deadline: None,
            },
            provenance: Provenance::Observed,
        })),
        EventKind::StateCorrectedFromObservation => {
            Some(Event::Fact(FactEvent::StateCorrectedFromObservation {
                entity_id: "_".into(),
                expected: LightActuatorState::On,
                observed: LightActuatorState::Off,
                provenance: Provenance::Derived,
            }))
        }
        EventKind::TemperatureReading => {
            Some(Event::Observation(ObservationEvent::TemperatureReading {
                sensor_id: "_".into(),
                millidegrees_c: 21000,
            }))
        }
        EventKind::ContactChanged => {
            Some(Event::Observation(ObservationEvent::ContactChanged {
                sensor_id: "_".into(),
                open: true,
            }))
        }
        EventKind::TemperatureRecorded => {
            Some(Event::Fact(FactEvent::TemperatureRecorded {
                sensor_id: "_".into(),
                millidegrees_c: 21000,
                provenance: Provenance::Observed,
            }))
        }
        EventKind::ContactStateChanged => {
            Some(Event::Fact(FactEvent::ContactStateChanged {
                sensor_id: "_".into(),
                open: true,
                provenance: Provenance::Observed,
            }))
        }
        EventKind::HumidityReading => Some(Event::Observation(ObservationEvent::HumidityReading {
            sensor_id: "_".into(),
            permille_rh: 500,
        })),
        EventKind::HumidityRecorded => Some(Event::Fact(FactEvent::HumidityRecorded {
            sensor_id: "_".into(),
            permille_rh: 500,
            provenance: Provenance::Observed,
        })),
    }
}

fn dfs_cycle(
    u: EventKind,
    adj: &HashMap<EventKind, Vec<EventKind>>,
    state: &mut HashMap<EventKind, u8>,
) -> bool {
    // 0 absent = unvisited, 1 = visiting, 2 = done
    match state.get(&u).copied().unwrap_or(0) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    state.insert(u, 1);
    if let Some(nei) = adj.get(&u) {
        for &v in nei {
            if dfs_cycle(v, adj, state) {
                return true;
            }
        }
    }
    state.insert(u, 2);
    false
}

impl Default for Registry {
    fn default() -> Self {
        Self::v0_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whitelist::ExceptionalFamilyTransition;
    use rusthome_core::{
        deterministic_command_id, CommandEvent, Event, FactEvent, ObservationEvent, Provenance,
        Rule, RuleContext,
    };

    static F2F_WHITELIST: &[ExceptionalFamilyTransition] = &[ExceptionalFamilyTransition {
        rule_id: "f2f",
        consumed_kind: EventKind::LightOn,
        produced_kind: EventKind::UsageLogged,
    }];

    #[test]
    fn v0_registry_no_cycle() {
        Registry::v0_default().validate_boot().unwrap();
    }

    #[test]
    fn minimal_registry_no_cycle() {
        Registry::minimal_default().validate_boot().unwrap();
    }

    #[test]
    fn home_registry_omits_r2_notify() {
        let v = Registry::v0_default();
        let h = Registry::home_default();
        let v_ids: Vec<_> = v.rules().iter().map(|r| r.rule_id()).collect();
        let h_ids: Vec<_> = h.rules().iter().map(|r| r.rule_id()).collect();
        assert_eq!(
            v_ids,
            vec![
                "R1", "R2", "R3", "R7", "R4", "R5", "R8", "R9", "R10", "R11", "R12", "R13"
            ]
        );
        assert_eq!(
            h_ids,
            vec!["R1", "R3", "R7", "R4", "R5", "R8", "R9", "R10", "R11", "R12", "R13"]
        );
    }

    struct GreedyRule;

    impl Rule for GreedyRule {
        fn rule_id(&self) -> &str {
            "greedy"
        }
        fn priority(&self) -> i32 {
            0
        }
        fn consumes(&self) -> &[EventKind] {
            const C: &[EventKind] = &[EventKind::MotionDetected];
            C
        }
        fn produces(&self) -> &[EventKind] {
            const P: &[EventKind] = &[EventKind::TurnOnLight];
            P
        }
        fn namespaces(&self) -> Vec<&str> {
            vec!["test"]
        }
        fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
            match event {
                Event::Observation(ObservationEvent::MotionDetected { .. }) => {
                    let command_id = deterministic_command_id(
                        "greedy",
                        "notify_user",
                        ctx.parent_sequence,
                        ctx.causal_chain_id,
                        "",
                    );
                    vec![Event::Command(CommandEvent::NotifyUser { command_id })]
                }
                _ => vec![],
            }
        }
    }

    #[test]
    fn boot_rejects_emission_outside_produces() {
        let reg = Registry::from_rules(vec![Arc::new(GreedyRule)], &[]);
        let err = reg.validate_boot().unwrap_err();
        match err {
            RegistryError::EmissionNotDeclared { kind, .. } => {
                assert_eq!(kind, EventKind::NotifyUser);
            }
            e => panic!("unexpected err: {e:?}"),
        }
    }

    struct FactToFactRule;

    impl Rule for FactToFactRule {
        fn rule_id(&self) -> &str {
            "f2f"
        }
        fn priority(&self) -> i32 {
            0
        }
        fn consumes(&self) -> &[EventKind] {
            const C: &[EventKind] = &[EventKind::LightOn];
            C
        }
        fn produces(&self) -> &[EventKind] {
            const P: &[EventKind] = &[EventKind::UsageLogged];
            P
        }
        fn namespaces(&self) -> Vec<&str> {
            vec!["test"]
        }
        fn eval(&self, event: &Event, _ctx: &RuleContext<'_>) -> Vec<Event> {
            match event {
                Event::Fact(FactEvent::LightOn { room, .. }) => {
                    vec![Event::Fact(FactEvent::UsageLogged {
                        item: room.clone(),
                        provenance: Provenance::Derived,
                    })]
                }
                _ => vec![],
            }
        }
    }

    #[test]
    fn boot_rejects_fact_to_fact_family() {
        let reg = Registry::from_rules(vec![Arc::new(FactToFactRule)], &[]);
        assert!(matches!(
            reg.validate_boot(),
            Err(RegistryError::FamilyTransitionForbidden { .. })
        ));
    }

    #[test]
    fn boot_accepts_fact_to_fact_with_whitelist() {
        let reg = Registry::from_rules(vec![Arc::new(FactToFactRule)], F2F_WHITELIST);
        reg.validate_boot().unwrap();
    }

    /// Rule with `rule_id` and owned lists — same shape as a "user" rule loaded at startup.
    struct RuntimeOwnedRule {
        id: String,
        consumes: Vec<EventKind>,
        produces: Vec<EventKind>,
    }

    impl Rule for RuntimeOwnedRule {
        fn rule_id(&self) -> &str {
            &self.id
        }
        fn priority(&self) -> i32 {
            0
        }
        fn consumes(&self) -> &[EventKind] {
            &self.consumes
        }
        fn produces(&self) -> &[EventKind] {
            &self.produces
        }
        fn namespaces(&self) -> Vec<&str> {
            vec!["user"]
        }
        fn eval(&self, _event: &Event, _ctx: &RuleContext<'_>) -> Vec<Event> {
            vec![]
        }
    }

    #[test]
    fn from_rules_accepts_runtime_owned_metadata() {
        let reg = Registry::from_rules(
            vec![Arc::new(RuntimeOwnedRule {
                id: "user.no_op".into(),
                consumes: vec![EventKind::MotionDetected],
                produces: vec![EventKind::TurnOnLight],
            })],
            &[],
        );
        reg.validate_boot().unwrap();
    }

    #[test]
    fn whitelist_redundant_entry_fails_boot() {
        static BAD: &[ExceptionalFamilyTransition] = &[ExceptionalFamilyTransition {
            rule_id: "greedy",
            consumed_kind: EventKind::MotionDetected,
            produced_kind: EventKind::TurnOnLight,
        }];
        let reg = Registry::from_rules(vec![Arc::new(GreedyRule)], BAD);
        assert!(matches!(
            reg.validate_boot(),
            Err(RegistryError::WhitelistRedundant { .. })
        ));
    }
}
