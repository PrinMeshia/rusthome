//! Registry presets — single entry point for the CLI and future loaders (file, plugins).

use std::fmt;
use std::str::FromStr;

use crate::{Registry, RegistryError};

/// Rule bundle identified by a stable name (snapshot digest, docs, support).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RulesPreset {
    /// Demo rules §16: motion → light → log (R1–R5).
    #[default]
    V0,
    /// R1+R3+R4+R5 (no R2 notify); digest `rules-home` for prod snapshots.
    Home,
    /// Motion → light + IO only (R1 + R3), without notify or usage log.
    Minimal,
}

impl RulesPreset {
    /// CLI / config identifier (case-insensitive).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V0 => "v0",
            Self::Home => "home",
            Self::Minimal => "minimal",
        }
    }

    /// Values accepted by [`FromStr`] (CLI / config help).
    pub const PRESET_IDS: &'static str = "v0, home, minimal";

    /// Default snapshot digest (§8.4) unless the subcommand sets `--snapshot-rules-digest` / `--rules-digest`.
    pub fn default_rules_digest(self) -> &'static str {
        match self {
            Self::V0 => "rules-v0",
            Self::Home => "rules-home",
            Self::Minimal => "rules-minimal",
        }
    }

    /// Builds the registry and runs [`Registry::validate_boot`].
    pub fn load_registry(self) -> Result<Registry, RegistryError> {
        let reg = match self {
            Self::V0 => Registry::v0_default(),
            Self::Home => Registry::home_default(),
            Self::Minimal => Registry::minimal_default(),
        };
        reg.validate_boot()?;
        Ok(reg)
    }
}

impl fmt::Display for RulesPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RulesPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "v0" => Ok(Self::V0),
            "home" => Ok(Self::Home),
            "minimal" => Ok(Self::Minimal),
            other => Err(format!(
                "unknown rules preset {other:?} (expected one of: {})",
                Self::PRESET_IDS
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v0_loads() {
        RulesPreset::V0.load_registry().unwrap();
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("V0".parse::<RulesPreset>().unwrap(), RulesPreset::V0);
    }

    #[test]
    fn minimal_loads() {
        RulesPreset::Minimal.load_registry().unwrap();
    }

    #[test]
    fn parse_minimal() {
        assert_eq!("minimal".parse::<RulesPreset>().unwrap(), RulesPreset::Minimal);
    }

    #[test]
    fn default_digest_matches_preset() {
        assert_eq!(RulesPreset::V0.default_rules_digest(), "rules-v0");
        assert_eq!(RulesPreset::Home.default_rules_digest(), "rules-home");
        assert_eq!(RulesPreset::Minimal.default_rules_digest(), "rules-minimal");
    }

    #[test]
    fn home_loads() {
        RulesPreset::Home.load_registry().unwrap();
    }

    #[test]
    fn parse_home() {
        assert_eq!("home".parse::<RulesPreset>().unwrap(), RulesPreset::Home);
    }
}
