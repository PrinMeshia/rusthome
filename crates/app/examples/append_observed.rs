//! Append an **Observed** light fact via the library API — template for a real adapter (MQTT, GPIO, etc.).
//!
//! Equivalent behaviour to `rusthome observed-light`, but as code you can copy into a sidecar process.
//!
//! ```text
//! cargo run -p rusthome-app --example append_observed -- \
//!   --data-dir data --timestamp 100 --room hall --state off --rules-preset v0
//! ```
//!
//! Use the same `--data-dir` and `rusthome.toml` as the CLI if you rely on presets; this example only
//! reads `--rules-preset` and `--io-anchored` (no TOML merge — keep in sync manually or extend).

use std::path::PathBuf;

use clap::Parser;
use rusthome_app::{
    append_observed_light_fact, replay_state, ObservedLightAppend, RunLimits,
};
use rusthome_core::{ConfigSnapshot, PhysicalProjectionMode};
use rusthome_infra::Journal;
use rusthome_rules::RulesPreset;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(about = "Append Observed light state (library demo for edge adapters)")]
struct Args {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long)]
    timestamp: i64,
    #[arg(long)]
    room: String,
    /// `on` or `off`
    #[arg(long)]
    state: String,
    #[arg(long, default_value = "v0")]
    rules_preset: String,
    #[arg(long, default_value_t = false)]
    io_anchored: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let on = match args.state.to_ascii_lowercase().as_str() {
        "on" => true,
        "off" => false,
        other => return Err(format!("--state must be on or off, got {other}").into()),
    };

    let preset: RulesPreset = args
        .rules_preset
        .parse()
        .map_err(|e: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let registry = preset.load_registry()?;

    let mut config = ConfigSnapshot::default();
    if args.io_anchored {
        config.physical_projection_mode = PhysicalProjectionMode::IoAnchored;
    }

    let journal_path = args.data_dir.join("events.jsonl");
    let mut journal = Journal::open(&journal_path)?;
    let mut state = replay_state(&journal_path)?;

    let causal = Uuid::new_v4();
    append_observed_light_fact(
        &mut journal,
        &mut state,
        &registry,
        &config,
        ObservedLightAppend {
            timestamp: args.timestamp,
            causal_chain_id: causal,
            room: args.room.clone(),
            on,
            correlation_id: None,
            trace_id: None,
        },
        RunLimits::default(),
        None,
    )?;

    println!(
        "ok: Observed light room={} on={} causal_chain_id={}",
        args.room, on, causal
    );
    Ok(())
}
