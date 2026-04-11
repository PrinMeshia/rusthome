//! Append an **Observed** light fact via the library API — template for a real adapter (MQTT, GPIO, etc.).
//!
//! Equivalent behaviour to `rusthome observed-light`, but as code you can copy into a sidecar process.
//!
//! Loads `{data-dir}/rusthome.toml` like the CLI (`rules_preset`, `physical_projection_mode`,
//! `io_timeout_logical_delta`, `[run_limits]`). `--rules-preset` and `--io-anchored` override file
//! values the same way as global CLI flags.
//!
//! ```text
//! cargo run -p rusthome-app --example append_observed -- \
//!   --data-dir data --timestamp 100 --room hall --state off
//! ```
//!
//! ```text
//! cargo run -p rusthome-app --example append_observed -- \
//!   --data-dir data --timestamp 100 --room hall --state off --rules-preset v0
//! ```

use std::path::PathBuf;

use clap::Parser;
use rusthome_app::{
    append_observed_light_fact, replay_state, rusthome_file, ObservedLightAppend,
};
use rusthome_infra::Journal;
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
    /// Overrides `rules_preset` from `rusthome.toml` when set (same as CLI `--rules-preset`).
    #[arg(long = "rules-preset")]
    rules_preset: Option<String>,
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

    let file = rusthome_file::load_rusthome_file(&args.data_dir)?;
    let preset = rusthome_file::resolve_rules_preset(args.rules_preset.as_deref(), &file)?;
    let registry = preset.load_registry()?;
    let config = rusthome_file::build_runtime_config(&file, args.io_anchored);
    let limits = rusthome_file::build_run_limits(&file);

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
        limits,
        None,
    )?;

    println!(
        "ok: Observed light room={} on={} causal_chain_id={}",
        args.room, on, causal
    );
    Ok(())
}
