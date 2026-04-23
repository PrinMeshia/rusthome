//! `rusthome init` — writes starter `rusthome.toml` and a Zigbee2MQTT YAML template into the data dir.

use std::path::Path;

const RUSTHOME_INIT_TOML: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/rusthome.init.toml"));
const Z2M_SUGGESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/zigbee2mqtt.configuration.example.yaml"
));

/// Writes each file if missing, or overwrites when `force` is true.
pub fn run(data_dir: &Path, force: bool) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(data_dir)?;
    write_one(
        &data_dir.join("rusthome.toml"),
        RUSTHOME_INIT_TOML,
        "rules + [zigbee2mqtt]",
        force,
    )?;
    write_one(
        &data_dir.join("zigbee2mqtt.configuration.suggested.yaml"),
        Z2M_SUGGESTED,
        "Zigbee2MQTT template (point your Z2M install or Docker -v at this file)",
        force,
    )?;
    eprintln!("init: next — set `serial.port` in zigbee2mqtt.configuration.suggested.yaml,");
    eprintln!("init:        then `rusthome serve` and start Zigbee2MQTT with that file (or copy it to your Z2M config path).");
    Ok(())
}

fn write_one(
    path: &Path,
    content: &str,
    label: &str,
    force: bool,
) -> Result<(), std::io::Error> {
    if path.exists() && !force {
        eprintln!("init: skip existing {} — {}", path.display(), label);
        return Ok(());
    }
    if path.exists() && force {
        eprintln!("init: overwrite (--force) {}", path.display());
    } else {
        eprintln!("init: wrote {} — {}", path.display(), label);
    }
    std::fs::write(path, content)
}
