//! TOML configuration for `rusthome-bridge`.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub broker: BrokerConfig,
    pub z2m: Z2mConfig,
    #[serde(default)]
    pub devices: Vec<DeviceRule>,
}

#[derive(Debug, Deserialize)]
pub struct BrokerConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: Option<String>,
    /// Plain password in file (avoid in production; prefer `password_env`).
    pub password: Option<String>,
    /// If set, read password from this environment variable when present (non-empty).
    pub password_env: Option<String>,
}

fn default_port() -> u16 {
    1883
}

#[derive(Debug, Deserialize)]
pub struct Z2mConfig {
    #[serde(default = "default_z2m_prefix")]
    pub topic_prefix: String,
}

fn default_z2m_prefix() -> String {
    "zigbee2mqtt".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceRule {
    pub match_friendly_name: String,
    pub mapping: Vec<FieldMapping>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldMapping {
    /// JSON key in the Zigbee2MQTT state object (e.g. `temperature`, `occupancy`).
    pub json_key: String,
    /// `temperature` | `humidity` | `contact` | `motion`
    pub family: String,
    /// Last segment of the rusthome topic `sensors/<family>/<rusthome_id>`.
    pub rusthome_id: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let c: Config = toml::from_str(&raw).context("parse bridge TOML")?;
        c.validate()?;
        Ok(c)
    }

    fn validate(&self) -> Result<()> {
        if self.devices.is_empty() {
            anyhow::bail!("at least one [[devices]] entry is required");
        }
        for d in &self.devices {
            if d.match_friendly_name.trim().is_empty() {
                anyhow::bail!("match_friendly_name must not be empty");
            }
            if d.mapping.is_empty() {
                anyhow::bail!(
                    "device {:?} must have at least one mapping",
                    d.match_friendly_name
                );
            }
            for m in &d.mapping {
                validate_rusthome_id(&m.rusthome_id)?;
                let fam = m.family.to_lowercase();
                if !matches!(
                    fam.as_str(),
                    "temperature" | "humidity" | "contact" | "motion"
                ) {
                    anyhow::bail!(
                        "unknown family {:?} for device {:?}",
                        m.family,
                        d.match_friendly_name
                    );
                }
            }
        }
        Ok(())
    }

    pub fn resolve_password(&self) -> Option<String> {
        if let Some(key) = &self.broker.password_env {
            if let Ok(v) = std::env::var(key) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        self.broker.password.clone()
    }
}

fn validate_rusthome_id(id: &str) -> Result<()> {
    let id = id.trim();
    if id.is_empty() || id.len() > 128 {
        anyhow::bail!("rusthome_id must be 1..=128 non-whitespace chars");
    }
    if id
        .chars()
        .any(|c| c == '/' || c == '+' || c == '#' || c.is_whitespace())
    {
        anyhow::bail!("rusthome_id must not contain spaces or / + #");
    }
    Ok(())
}
