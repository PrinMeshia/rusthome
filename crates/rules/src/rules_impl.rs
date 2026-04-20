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

/// `TurnOffLight` command → `LightOff` + `CommandIo` (same IO shape as R3).
pub struct R7;

impl Rule for R7 {
    fn rule_id(&self) -> &str {
        "R7"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        const C: &[EventKind] = &[EventKind::TurnOffLight];
        C
    }

    fn produces(&self) -> &[EventKind] {
        const P: &[EventKind] = &[EventKind::LightOff, EventKind::CommandIo];
        P
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["lighting"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Command(CommandEvent::TurnOffLight { room, command_id }) => {
                let prov = match ctx.config.physical_projection_mode {
                    PhysicalProjectionMode::Simulation => Provenance::Derived,
                    PhysicalProjectionMode::IoAnchored => Provenance::Derived,
                };
                let deadline = ctx
                    .trigger_timestamp
                    .checked_add(ctx.config.io_timeout_logical_delta);
                let room = room.clone();
                let cid = *command_id;
                vec![
                    Event::Fact(FactEvent::LightOff {
                        room: room.clone(),
                        provenance: prov,
                    }),
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

pub static R7_TURNOFF_FACT: R7 = R7;

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

/// TemperatureReading → TemperatureRecorded (Observation → Fact; whitelisted §6.17).
pub struct R8;

impl Rule for R8 {
    fn rule_id(&self) -> &str {
        "R8"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        &[EventKind::TemperatureReading]
    }

    fn produces(&self) -> &[EventKind] {
        &[EventKind::TemperatureRecorded]
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["sensors"]
    }

    fn eval(&self, event: &Event, _ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Observation(ObservationEvent::TemperatureReading {
                sensor_id,
                millidegrees_c,
            }) => {
                vec![Event::Fact(FactEvent::TemperatureRecorded {
                    sensor_id: sensor_id.clone(),
                    millidegrees_c: *millidegrees_c,
                    provenance: Provenance::Observed,
                })]
            }
            _ => vec![],
        }
    }
}

pub static R8_TEMPERATURE_FACT: R8 = R8;

/// ContactChanged → ContactStateChanged (Observation → Fact; whitelisted §6.17).
pub struct R9;

impl Rule for R9 {
    fn rule_id(&self) -> &str {
        "R9"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        &[EventKind::ContactChanged]
    }

    fn produces(&self) -> &[EventKind] {
        &[EventKind::ContactStateChanged]
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["sensors"]
    }

    fn eval(&self, event: &Event, _ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Observation(ObservationEvent::ContactChanged { sensor_id, open }) => {
                vec![Event::Fact(FactEvent::ContactStateChanged {
                    sensor_id: sensor_id.clone(),
                    open: *open,
                    provenance: Provenance::Observed,
                })]
            }
            _ => vec![],
        }
    }
}

pub static R9_CONTACT_FACT: R9 = R9;

/// TemperatureRecorded → LogUsage (log temperature readings).
pub struct R10;

impl Rule for R10 {
    fn rule_id(&self) -> &str {
        "R10"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        &[EventKind::TemperatureRecorded]
    }

    fn produces(&self) -> &[EventKind] {
        &[EventKind::LogUsage]
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["logging"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Fact(FactEvent::TemperatureRecorded { sensor_id, .. }) => {
                let item = format!("temperature:{sensor_id}");
                let command_id = deterministic_command_id(
                    "R10",
                    "log_usage",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    &item,
                );
                vec![Event::Command(CommandEvent::LogUsage { item, command_id })]
            }
            _ => vec![],
        }
    }
}

pub static R10_TEMPERATURE_LOG: R10 = R10;

/// ContactStateChanged → LogUsage (log contact sensor changes).
pub struct R11;

impl Rule for R11 {
    fn rule_id(&self) -> &str {
        "R11"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        &[EventKind::ContactStateChanged]
    }

    fn produces(&self) -> &[EventKind] {
        &[EventKind::LogUsage]
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["logging"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Fact(FactEvent::ContactStateChanged { sensor_id, .. }) => {
                let item = format!("contact:{sensor_id}");
                let command_id = deterministic_command_id(
                    "R11",
                    "log_usage",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    &item,
                );
                vec![Event::Command(CommandEvent::LogUsage { item, command_id })]
            }
            _ => vec![],
        }
    }
}

pub static R11_CONTACT_LOG: R11 = R11;

/// HumidityReading → HumidityRecorded (Observation → Fact; same pattern as R8).
pub struct R12;

impl Rule for R12 {
    fn rule_id(&self) -> &str {
        "R12"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        &[EventKind::HumidityReading]
    }

    fn produces(&self) -> &[EventKind] {
        &[EventKind::HumidityRecorded]
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["sensors"]
    }

    fn eval(&self, event: &Event, _ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Observation(ObservationEvent::HumidityReading {
                sensor_id,
                permille_rh,
            }) => {
                vec![Event::Fact(FactEvent::HumidityRecorded {
                    sensor_id: sensor_id.clone(),
                    permille_rh: *permille_rh,
                    provenance: Provenance::Observed,
                })]
            }
            _ => vec![],
        }
    }
}

pub static R12_HUMIDITY_FACT: R12 = R12;

/// HumidityRecorded → LogUsage.
pub struct R13;

impl Rule for R13 {
    fn rule_id(&self) -> &str {
        "R13"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn consumes(&self) -> &[EventKind] {
        &[EventKind::HumidityRecorded]
    }

    fn produces(&self) -> &[EventKind] {
        &[EventKind::LogUsage]
    }

    fn namespaces(&self) -> Vec<&str> {
        vec!["logging"]
    }

    fn eval(&self, event: &Event, ctx: &RuleContext<'_>) -> Vec<Event> {
        match event {
            Event::Fact(FactEvent::HumidityRecorded { sensor_id, .. }) => {
                let item = format!("humidity:{sensor_id}");
                let command_id = deterministic_command_id(
                    "R13",
                    "log_usage",
                    ctx.parent_sequence,
                    ctx.causal_chain_id,
                    &item,
                );
                vec![Event::Command(CommandEvent::LogUsage { item, command_id })]
            }
            _ => vec![],
        }
    }
}

pub static R13_HUMIDITY_LOG: R13 = R13;
