//! V0 rules — §16 scenario (R1–R5 + R7), presets and registry.

mod bundle;
mod family_transitions;
mod preset;
mod registry;
mod rules_impl;
mod whitelist;

pub use preset::RulesPreset;
pub use registry::{Registry, RegistryError, MAX_CONSUMED_EVENT_TYPES_PER_RULE};
pub use rules_impl::{
    R10_TEMPERATURE_LOG, R11_CONTACT_LOG, R12_HUMIDITY_FACT, R13_HUMIDITY_LOG, R1_MOTION_LIGHT,
    R2_MOTION_NOTIFY, R3_TURNON_FACT, R4_LIGHT_LOG, R5_LOG_USAGE_FACT, R7_TURNOFF_FACT,
    R8_TEMPERATURE_FACT, R9_CONTACT_FACT,
};
pub use whitelist::ExceptionalFamilyTransition;
