//! Append a **`TurnOffLight` command** via the library API — template for a switch / scene adapter.
//!
//! Same rule cascade as CLI `rusthome turn-off-light` (R7 in all presets).
//!
//! Loads `{data-dir}/rusthome.toml` like the CLI (`rules_preset`, `physical_projection_mode`,
//! `io_timeout_logical_delta`, `[run_limits]`). Optional `--rules-preset` / `--io-anchored` override
//! the file like global CLI flags.
//!
//! Optional `--command-id` and `--causal-chain-id` (UUID strings) match the CLI for reproducible
//! journals and §14.3 dedup.
//!
//! ```text
//! cargo run -p rusthome-app --example ingest_turn_off -- \
//!   --data-dir data --timestamp 200 --room hall
//! ```
//!
//! ```text
//! cargo run -p rusthome-app --example ingest_turn_off -- \
//!   --data-dir data --timestamp 200 --room hall --rules-preset minimal
//! ```

use std::path::PathBuf;

use clap::Parser;
use rusthome_app::{ingest_command_with_causal, replay_state, rusthome_file};
use rusthome_core::CommandEvent;
use rusthome_infra::Journal;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(about = "Ingest TurnOffLight command (library demo for adapters)")]
struct Args {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long)]
    timestamp: i64,
    #[arg(long)]
    room: String,
    #[arg(long = "rules-preset")]
    rules_preset: Option<String>,
    #[arg(long, default_value_t = false)]
    io_anchored: bool,
    #[arg(long = "command-id")]
    command_id: Option<String>,
    #[arg(long = "causal-chain-id")]
    causal_chain_id: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let file = rusthome_file::load_rusthome_file(&args.data_dir)?;
    let preset = rusthome_file::resolve_rules_preset(args.rules_preset.as_deref(), &file)?;
    let registry = preset.load_registry()?;
    let config = rusthome_file::build_runtime_config(&file, args.io_anchored);
    let limits = rusthome_file::build_run_limits(&file);

    let command_uuid = match args.command_id.as_deref() {
        Some(s) => Uuid::parse_str(s).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid --command-id: {e}"),
            )
        })?,
        None => Uuid::new_v4(),
    };
    let causal = match args.causal_chain_id.as_deref() {
        Some(s) => Uuid::parse_str(s).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid --causal-chain-id: {e}"),
            )
        })?,
        None => Uuid::new_v4(),
    };

    let journal_path = args.data_dir.join("events.jsonl");
    let mut journal = Journal::open(&journal_path)?;
    let mut state = replay_state(&journal_path)?;

    ingest_command_with_causal(
        &mut journal,
        &mut state,
        &registry,
        &config,
        args.timestamp,
        CommandEvent::TurnOffLight {
            room: args.room.clone(),
            command_id: command_uuid,
        },
        causal,
        limits,
    )?;

    println!(
        "ok: TurnOffLight room={} command_id={} causal_chain_id={}",
        args.room, command_uuid, causal
    );
    Ok(())
}
