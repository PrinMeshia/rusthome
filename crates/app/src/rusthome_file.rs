//! Optional `{data-dir}/rusthome.toml` — rules preset + runtime parameters ([`ConfigSnapshot`](rusthome_core::ConfigSnapshot)).
//!
//! Shared by the CLI and library examples so adapters match `rusthome …` behaviour.

use std::path::Path;

use serde::Deserialize;

use crate::RunLimits;
use rusthome_core::{ConfigSnapshot, PhysicalProjectionMode};
use rusthome_rules::RulesPreset;

/// Optional `[zigbee2mqtt]` — MQTT bridge control when Zigbee2MQTT shares the same broker as `rusthome serve`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Zigbee2MqttConfig {
    /// Topic prefix (e.g. `zigbee2mqtt`). Published: `{prefix}/bridge/request`.
    #[serde(default)]
    pub mqtt_topic_prefix: Option<String>,
    /// Default duration for permit join (seconds), 1–900.
    #[serde(default)]
    pub permit_join_seconds: Option<u64>,
}

impl Zigbee2MqttConfig {
    /// Effective MQTT topic prefix (non-empty).
    pub fn resolved_topic_prefix(&self) -> String {
        self.mqtt_topic_prefix
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("zigbee2mqtt")
            .to_string()
    }

    /// Default permit-join duration for API/UI.
    pub fn resolved_permit_join_seconds(&self) -> u64 {
        self.permit_join_seconds.unwrap_or(120).clamp(1, 900)
    }
}

/// `[run_limits]` sub-table — plan §6.6; omitted fields use engine defaults.
#[derive(Debug, Default, Deserialize)]
pub struct RunLimitsConfig {
    #[serde(default)]
    pub max_events_per_run: Option<u64>,
    #[serde(default)]
    pub max_events_generated_per_root: Option<u64>,
    #[serde(default)]
    pub max_wall_ms_per_run: Option<u64>,
    #[serde(default)]
    pub max_pending_events: Option<usize>,
}

/// Parsed `rusthome.toml` content (all fields optional).
#[derive(Debug, Default, Deserialize)]
pub struct RusthomeFile {
    #[serde(default)]
    pub rules_preset: Option<String>,
    /// `simulation` or `io_anchored` (snake_case). Used when the caller does not force IoAnchored.
    #[serde(default)]
    pub physical_projection_mode: Option<String>,
    /// §6.16 — logical time delta for `Dispatched.logical_deadline`.
    #[serde(default)]
    pub io_timeout_logical_delta: Option<i64>,
    #[serde(default)]
    pub run_limits: Option<RunLimitsConfig>,
    #[serde(default)]
    pub zigbee2mqtt: Option<Zigbee2MqttConfig>,
}

pub const PHYSICAL_MODES_HELP: &str = "simulation, io_anchored";

/// Validates set fields (typos, values out of domain).
pub fn validate_rusthome_file(file: &RusthomeFile) -> Result<(), String> {
    if let Some(ref s) = file.rules_preset {
        let t = s.trim();
        if !t.is_empty() {
            let _: RulesPreset = t.parse().map_err(|e| {
                format!("rules_preset: {e} (expected one of: {})", RulesPreset::PRESET_IDS)
            })?;
        }
    }
    if let Some(ref s) = file.physical_projection_mode {
        let t = s.trim();
        if !t.is_empty() && parse_physical_projection_mode(t).is_none() {
            return Err(format!(
                "physical_projection_mode: unknown value {t:?} (expected {PHYSICAL_MODES_HELP})"
            ));
        }
    }
    if let Some(d) = file.io_timeout_logical_delta {
        if d <= 0 {
            return Err(format!("io_timeout_logical_delta: must be > 0, got {d}"));
        }
    }
    if let Some(ref rl) = file.run_limits {
        validate_run_limits_config(rl)?;
    }
    if let Some(ref z) = file.zigbee2mqtt {
        validate_zigbee2mqtt_config(z)?;
    }
    Ok(())
}

fn validate_zigbee2mqtt_config(z: &Zigbee2MqttConfig) -> Result<(), String> {
    if let Some(ref p) = z.mqtt_topic_prefix {
        let t = p.trim();
        if t.is_empty() {
            return Err("zigbee2mqtt.mqtt_topic_prefix: must not be empty when set".into());
        }
        if t.contains('#') || t.contains('+') || t.contains(' ') {
            return Err(
                "zigbee2mqtt.mqtt_topic_prefix: must not contain #, +, or spaces".into(),
            );
        }
    }
    if let Some(s) = z.permit_join_seconds {
        if s == 0 || s > 900 {
            return Err(format!(
                "zigbee2mqtt.permit_join_seconds: must be between 1 and 900, got {s}"
            ));
        }
    }
    Ok(())
}

fn validate_run_limits_config(rl: &RunLimitsConfig) -> Result<(), String> {
    if let Some(v) = rl.max_events_per_run {
        if v == 0 {
            return Err("run_limits.max_events_per_run: must be > 0".into());
        }
    }
    if let Some(v) = rl.max_events_generated_per_root {
        if v == 0 {
            return Err("run_limits.max_events_generated_per_root: must be > 0".into());
        }
    }
    if let Some(v) = rl.max_wall_ms_per_run {
        if v == 0 {
            return Err("run_limits.max_wall_ms_per_run: must be > 0".into());
        }
    }
    if let Some(v) = rl.max_pending_events {
        if v == 0 {
            return Err("run_limits.max_pending_events: must be > 0".into());
        }
    }
    Ok(())
}

/// Merges `[run_limits]` with [`RunLimits`] defaults (§6.6).
pub fn build_run_limits(file: &RusthomeFile) -> RunLimits {
    let mut lim = RunLimits::default();
    let Some(ref rl) = file.run_limits else {
        return lim;
    };
    if let Some(v) = rl.max_events_per_run {
        lim.max_events_per_run = v;
    }
    if let Some(v) = rl.max_events_generated_per_root {
        lim.max_events_generated_per_root = v;
    }
    if let Some(v) = rl.max_wall_ms_per_run {
        lim.max_wall_ms_per_run = v;
    }
    if let Some(v) = rl.max_pending_events {
        lim.max_pending_events = v;
    }
    lim
}

/// Missing file → defaults; present → strict parse + validation.
pub fn load_rusthome_file(data_dir: &Path) -> Result<RusthomeFile, std::io::Error> {
    let path = data_dir.join("rusthome.toml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Ok(RusthomeFile::default());
    };
    let file: RusthomeFile = toml::from_str(&raw).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {e}", path.display()),
        )
    })?;
    validate_rusthome_file(&file).map_err(|msg| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {msg}", path.display()),
        )
    })?;
    Ok(file)
}

fn parse_physical_projection_mode(s: &str) -> Option<PhysicalProjectionMode> {
    match s.trim().to_ascii_lowercase().as_str() {
        "simulation" => Some(PhysicalProjectionMode::Simulation),
        "io_anchored" => Some(PhysicalProjectionMode::IoAnchored),
        _ => None,
    }
}

/// `io_anchored_cli` forces [`PhysicalProjectionMode::IoAnchored`]; else file then default simulation.
pub fn build_runtime_config(file: &RusthomeFile, io_anchored_cli: bool) -> ConfigSnapshot {
    let mut cfg = ConfigSnapshot::default();
    if let Some(d) = file.io_timeout_logical_delta {
        cfg.io_timeout_logical_delta = d;
    }
    cfg.physical_projection_mode = if io_anchored_cli {
        PhysicalProjectionMode::IoAnchored
    } else {
        file.physical_projection_mode
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(parse_physical_projection_mode)
            .unwrap_or(PhysicalProjectionMode::Simulation)
    };
    cfg
}

/// Order: `cli_preset` if non-empty, else `file.rules_preset`, else `v0`.
pub fn resolve_rules_preset(
    cli_preset: Option<&str>,
    file: &RusthomeFile,
) -> Result<RulesPreset, std::io::Error> {
    let s = cli_preset
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            file.rules_preset
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| RulesPreset::V0.as_str().to_string());
    s.parse()
        .map_err(|msg| std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))
}

/// Snapshot digest §8.4: CLI value if set, else default for the preset.
pub fn resolve_rules_digest(cli_digest: Option<&str>, preset: RulesPreset) -> String {
    match cli_digest.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => preset.default_rules_digest().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_toml_selects_minimal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"rules_preset = "minimal""#,
        )
        .unwrap();
        let file = load_rusthome_file(dir.path()).unwrap();
        let p = resolve_rules_preset(None, &file).unwrap();
        assert_eq!(p, RulesPreset::Minimal);
    }

    #[test]
    fn cli_overrides_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"rules_preset = "minimal""#,
        )
        .unwrap();
        let file = load_rusthome_file(dir.path()).unwrap();
        let p = resolve_rules_preset(Some("v0"), &file).unwrap();
        assert_eq!(p, RulesPreset::V0);
    }

    #[test]
    fn io_timeout_from_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"io_timeout_logical_delta = 120"#,
        )
        .unwrap();
        let file = load_rusthome_file(dir.path()).unwrap();
        let cfg = build_runtime_config(&file, false);
        assert_eq!(cfg.io_timeout_logical_delta, 120);
    }

    #[test]
    fn io_anchored_cli_wins_over_file_simulation() {
        let f = RusthomeFile {
            physical_projection_mode: Some("simulation".into()),
            ..Default::default()
        };
        validate_rusthome_file(&f).unwrap();
        let cfg = build_runtime_config(&f, true);
        assert_eq!(cfg.physical_projection_mode, PhysicalProjectionMode::IoAnchored);
    }

    #[test]
    fn physical_mode_from_file_when_cli_false() {
        let f = RusthomeFile {
            physical_projection_mode: Some("io_anchored".into()),
            ..Default::default()
        };
        validate_rusthome_file(&f).unwrap();
        let cfg = build_runtime_config(&f, false);
        assert_eq!(cfg.physical_projection_mode, PhysicalProjectionMode::IoAnchored);
    }

    #[test]
    fn invalid_toml_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rusthome.toml"), "not[[toml").unwrap();
        let e = load_rusthome_file(dir.path()).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn unknown_physical_mode_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"physical_projection_mode = "banana""#,
        )
        .unwrap();
        let e = load_rusthome_file(dir.path()).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            e.to_string().contains("physical_projection_mode"),
            "{}",
            e
        );
    }

    #[test]
    fn non_positive_io_timeout_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"io_timeout_logical_delta = 0"#,
        )
        .unwrap();
        let e = load_rusthome_file(dir.path()).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn bad_rules_preset_in_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"rules_preset = "nope""#,
        )
        .unwrap();
        let e = load_rusthome_file(dir.path()).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn run_limits_partial_merge() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"
[run_limits]
max_events_per_run = 2000
max_wall_ms_per_run = 5000
"#,
        )
        .unwrap();
        let file = load_rusthome_file(dir.path()).unwrap();
        let lim = build_run_limits(&file);
        assert_eq!(lim.max_events_per_run, 2000);
        assert_eq!(lim.max_wall_ms_per_run, 5000);
        assert_eq!(
            lim.max_events_generated_per_root,
            RunLimits::default().max_events_generated_per_root
        );
    }

    #[test]
    fn run_limits_zero_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"
[run_limits]
max_events_per_run = 0
"#,
        )
        .unwrap();
        let e = load_rusthome_file(dir.path()).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn zigbee2mqtt_defaults_resolve() {
        let z = Zigbee2MqttConfig::default();
        assert_eq!(z.resolved_topic_prefix(), "zigbee2mqtt");
        assert_eq!(z.resolved_permit_join_seconds(), 120);
    }

    #[test]
    fn zigbee2mqtt_bad_prefix_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rusthome.toml"),
            r#"
[zigbee2mqtt]
mqtt_topic_prefix = "bad#topic"
"#,
        )
        .unwrap();
        let e = load_rusthome_file(dir.path()).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    }
}
