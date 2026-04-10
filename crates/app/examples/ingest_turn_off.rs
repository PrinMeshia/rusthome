//! Append a **`TurnOffLight` command** via the library API — template for a switch / scene adapter.
//!
//! Same rule cascade as CLI `rusthome turn-off-light` (R7 in all presets).
//!
//! ```text
//! cargo run -p rusthome-app --example ingest_turn_off -- \
//!   --data-dir data --timestamp 200 --room hall --rules-preset minimal
//! ```
//!
//! Optional `--command-id` and `--causal-chain-id` (UUID strings) match the CLI for reproducible
//! journals and §14.3 dedup. This example does not load `rusthome.toml`; align with production manually
//! or reuse `crates/cli/src/config.rs` patterns (see [integration.md](../../docs/integration.md)).

use std::path::PathBuf;

use clap::Parser;
use rusthome_app::{ingest_command_with_causal, replay_state, RunLimits};
use rusthome_core::{CommandEvent, ConfigSnapshot, PhysicalProjectionMode};
use rusthome_infra::Journal;
use rusthome_rules::RulesPreset;
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
    #[arg(long, default_value = "minimal")]
    rules_preset: String,
    #[arg(long, default_value_t = false)]
    io_anchored: bool,
    #[arg(long = "command-id")]
    command_id: Option<String>,
    #[arg(long = "causal-chain-id")]
    causal_chain_id: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let preset: RulesPreset = args
        .rules_preset
        .parse()
        .map_err(|e: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let registry = preset.load_registry()?;

    let mut config = ConfigSnapshot::default();
    if args.io_anchored {
        config.physical_projection_mode = PhysicalProjectionMode::IoAnchored;
    }

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
        RunLimits::default(),
    )?;

    println!(
        "ok: TurnOffLight room={} command_id={} causal_chain_id={}",
        args.room, command_uuid, causal
    );
    Ok(())
}
