use rusthome_core::{
    deterministic_command_id, CommandEvent, CommandIoPhase, Event, EventKind, FactEvent,
    ObservationEvent, PhysicalProjectionMode, Provenance, Rule, RuleContext,
};

pub struct R1;

impl Rule for R1 {
    fn rule_id(&self) -> &str {
        "R1"
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
        vec!["lighting"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Observation(ObservationEvent::MotionDetected { room }) => {
                let command_id = deterministic_command_id(
                    "R1",
                    "turn_on_light",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    room.as_str(),
                );
                vec![Event::Command(CommandEvent::TurnOnLight {
                    room: room.clone(),
                    command_id,
                })]
            }
            _ => vec![],
        }
    }
}

pub struct R2;

impl Rule for R2 {
    fn rule_id(&self) -> &str {
        "R2"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        const C: &[EventKind] = &[EventKind::MotionDetected];
        C
    }

    fn produces(&self) -> &[EventKind] {
        const P: &[EventKind] = &[EventKind::NotifyUser];
        P
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["notify"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Observation(ObservationEvent::MotionDetected { room }) => {
                let command_id = deterministic_command_id(
                    "R2",
                    "notify_user",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    room.as_str(),
                );
                vec![Event::Command(CommandEvent::NotifyUser { command_id })]
            }
            _ => vec![],
        }
    }
}

pub struct R3;

impl Rule for R3 {
    fn rule_id(&self) -> &str {
        "R3"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        const C: &[EventKind] = &[EventKind::TurnOnLight];
        C
    }

    fn produces(&self) -> &[EventKind] {
        const P: &[EventKind] = &[EventKind::LightOn, EventKind::CommandIo];
        P
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["lighting"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Command(CommandEvent::TurnOnLight { room, command_id }) => {
                let prov = match ctx.config.physical_projection_mode {
                    PhysicalProjectionMode::Simulation => Provenance::Derived,
                    PhysicalProjectionMode::IoAnchored => {
                        // V0 demo: still derived unless IO layer emits ObservedFact separately
                        Provenance::Derived
                    }
                };
                let deadline = ctx
                    .trigger_timestamp
                    .checked_add(ctx.config.io_timeout_logical_delta);
                let room = room.clone();
                let cid = *command_id;
                vec![
                    Event::Fact(FactEvent::LightOn {
                        room: room.clone(),
                        provenance: prov,
                    }),
                    // EPIC 2 — never Acked without Dispatched in the same batch (shadow validate pipeline).
                    Event::Fact(FactEvent::CommandIo {
                        command_id: Some(cid),
                        room: Some(room.clone()),
                        phase: CommandIoPhase::Dispatched {
                            logical_deadline: deadline,
                        },
                        provenance: Provenance::Derived,
                    }),
                    Event::Fact(FactEvent::CommandIo {
                        command_id: Some(cid),
                        room: Some(room.clone()),
                        phase: CommandIoPhase::Acked,
                        provenance: Provenance::Derived,
                    }),
                ]
            }
            _ => vec![],
        }
    }
}

pub struct R4;

impl Rule for R4 {
    fn rule_id(&self) -> &str {
        "R4"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        const C: &[EventKind] = &[EventKind::LightOn];
        C
    }

    fn produces(&self) -> &[EventKind] {
        const P: &[EventKind] = &[EventKind::LogUsage];
        P
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["logging"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Fact(FactEvent::LightOn { room, .. }) => {
                let item = format!("light:{room}");
                let command_id = deterministic_command_id(
                    "R4",
                    "log_usage",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    item.as_str(),
                );
                vec![Event::Command(CommandEvent::LogUsage { item, command_id })]
            }
            _ => vec![],
        }
    }
}

pub static R1_MOTION_LIGHT: R1 = R1;
pub static R2_MOTION_NOTIFY: R2 = R2;
pub static R3_TURNON_FACT: R3 = R3;
pub static R4_LIGHT_LOG: R4 = R4;

pub struct R5;

impl Rule for R5 {
    fn rule_id(&self) -> &str {
        "R5"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        const C: &[EventKind] = &[EventKind::LogUsage];
        C
    }

    fn produces(&self) -> &[EventKind] {
        const P: &[EventKind] = &[EventKind::UsageLogged];
        P
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["logging"]
    }

    fn eval(&self, event: &Event, _ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Command(CommandEvent::LogUsage { item, .. }) => {
                vec![Event::Fact(FactEvent::UsageLogged {
                    item: item.clone(),
                    provenance: Provenance::Derived,
                })]
            }
            _ => vec![],
        }
    }
}

pub static R5_LOG_USAGE_FACT: R5 = R5;
