mod config;
mod init;
mod serve;

use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use rusthome_app::{
    append_observed_light_fact, ingest_command_with_causal, ingest_command_with_causal_traced,
    ingest_observation, ingest_observation_with_causal_traced, replay_state, ObservedLightAppend,
};
use rusthome_core::{CommandEvent, ObservationEvent, PhysicalProjectionMode, RuleEvaluationRecord};
use rusthome_infra::{
    load_and_sort, repair_journal, verify_contiguous_sequence, Journal, Snapshot,
};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "rusthome")]
#[command(about = "Deterministic home automation V0 (plan rusthome)")]
struct Cli {
    /// May be overridden with the `RUSTHOME_DATA_DIR` environment variable.
    #[arg(long, global = true, default_value = "data", env = "RUSTHOME_DATA_DIR")]
    data_dir: PathBuf,

    /// Rules bundle: `--rules-preset` > `RUSTHOME_RULES_PRESET` > `rusthome.toml` > `v0`.
    /// Values: `v0` (R1–R5 + R7 + notify), `home` (R1+R3+R7+R4+R5, digest `rules-home`), `minimal` (R1+R3+R7).
    #[arg(long = "rules-preset", global = true, env = "RUSTHOME_RULES_PRESET")]
    rules_preset: Option<String>,

    /// fsync after each journal line (§8.1 — more durable, slower).
    #[arg(long, global = true, default_value_t = false)]
    journal_fsync: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Append an observation and run rules (timestamp = logical time, required).
    Emit {
        #[arg(long)]
        timestamp: i64,
        #[arg(long, default_value = "hall")]
        room: String,
        /// IoAnchored mode: rejects Derived light facts emitted by rules (§14.5).
        #[arg(long, default_value_t = false)]
        io_anchored: bool,
        /// Append §15 trace (JSON Lines): one line per rule and per processed event.
        #[arg(long)]
        trace_file: Option<PathBuf>,
        /// Write `snapshot.json` after the cascade (§8.4 — recommended in prod after each journal commit).
        #[arg(long, default_value_t = false)]
        write_snapshot: bool,
        /// Default: `rules-v0` / `rules-home` / `rules-minimal` per preset.
        #[arg(long = "snapshot-rules-digest")]
        snapshot_rules_digest: Option<String>,
    },
    /// Append `TurnOffLight` command and run rules (R7 → `LightOff` + IO facts in Simulation).
    TurnOffLight {
        #[arg(long)]
        timestamp: i64,
        #[arg(long, default_value = "hall")]
        room: String,
        /// `command_id` for this line (default: random). Duplicate ids → no new line (§14.3).
        #[arg(long = "command-id")]
        command_id: Option<String>,
        /// `causal_chain_id` for this cascade (default: random). Set for reproducible journals / `explain`.
        #[arg(long = "causal-chain-id")]
        causal_chain_id: Option<String>,
        #[arg(long, default_value_t = false)]
        io_anchored: bool,
        #[arg(long)]
        trace_file: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        write_snapshot: bool,
        #[arg(long = "snapshot-rules-digest")]
        snapshot_rules_digest: Option<String>,
    },
    /// Print projection state (replay journal).
    State,
    /// Replay journal and compare state (determinism check).
    Replay,
    /// Write snapshot file after replay.
    Snapshot {
        /// Default: per preset (`rules-v0`, `rules-home`, `rules-minimal`).
        #[arg(long = "rules-digest")]
        rules_digest: Option<String>,
    },
    /// §8.5 — copy journal to backup, truncate after last valid JSON line.
    Repair {
        #[arg(long, default_value = ".bak")]
        backup_suffix: String,
    },
    /// Print journal entries whose `causal_chain_id` matches (§15 / §24).
    Explain {
        #[arg(long)]
        causal: String,
    },
    /// List rules + consumes→produces edges (Mermaid) — §23.
    RulesDoc,
    /// Micro-bench: N MotionDetected ingests on a temporary journal (§7.1).
    Bench {
        #[arg(long, default_value_t = 50)]
        count: u32,
    },
    /// Create starter files in the data dir: `rusthome.toml` + Zigbee2MQTT YAML template (skip if present; use `--force` to overwrite).
    Init {
        /// Overwrite existing `rusthome.toml` / `zigbee2mqtt.configuration.suggested.yaml`.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// All-in-one: embedded MQTT broker + ingest adapter + web dashboard.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
        /// TCP port for the embedded MQTT broker (0 = OS-assigned).
        #[arg(long, default_value_t = 1883)]
        mqtt_port: u16,
        /// MQTT topic filter for the ingest adapter.
        #[arg(long, default_value = "sensors/#")]
        mqtt_topic: String,
        /// Disable the embedded broker (web dashboard only, like previous behaviour).
        #[arg(long, default_value_t = false)]
        no_broker: bool,
    },
    /// Append an **Observed** light fact (reconciliation when Derived projection diverges).
    ObservedLight {
        #[arg(long)]
        timestamp: i64,
        #[arg(long)]
        room: String,
        #[arg(long, value_enum)]
        state: ObservedLightState,
        #[arg(long, default_value_t = false)]
        io_anchored: bool,
        #[arg(long, default_value_t = false)]
        write_snapshot: bool,
        /// Default: digest per preset (`rules-v0`, `rules-home`, `rules-minimal`).
        #[arg(long = "snapshot-rules-digest")]
        snapshot_rules_digest: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ObservedLightState {
    On,
    Off,
}

fn journal_path(data: &Path) -> PathBuf {
    data.join("events.jsonl")
}

fn snapshot_path(data: &Path) -> PathBuf {
    data.join("snapshot.json")
}

fn save_snapshot(data_dir: &Path, rules_digest: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = journal_path(data_dir);
    let entries = load_and_sort(&path)?;
    verify_contiguous_sequence(&entries)?;
    let last_seq = entries.last().map(|e| e.sequence).unwrap_or(0);
    let state = replay_state(&path)?;
    let snap = Snapshot::from_state(
        rusthome_core::SCHEMA_VERSION,
        last_seq,
        &state,
        rules_digest,
    );
    snap.save(snapshot_path(data_dir))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.data_dir)?;

    if let Commands::Init { force } = &cli.command {
        init::run(&cli.data_dir, *force)?;
        return Ok(());
    }

    if let Commands::Serve {
        bind,
        mqtt_port,
        mqtt_topic,
        no_broker,
    } = &cli.command
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let data_dir = cli.data_dir.clone();
        let bind = bind.clone();
        let mqtt_port = *mqtt_port;
        let mqtt_topic = mqtt_topic.clone();
        let no_broker = *no_broker;

        rt.block_on(async move {
            serve::serve_all_in_one(data_dir, bind, mqtt_port, mqtt_topic, no_broker).await
        })?;
        return Ok(());
    }

    let rusthome_file = config::load_rusthome_file(&cli.data_dir)?;
    let preset = config::resolve_rules_preset(cli.rules_preset.as_deref(), &rusthome_file)?;
    let registry = preset.load_registry()?;
    let run_limits = config::build_run_limits(&rusthome_file);

    match cli.command {
        Commands::Emit {
            timestamp,
            room,
            io_anchored,
            trace_file,
            write_snapshot,
            snapshot_rules_digest,
        } => {
            let config = config::build_runtime_config(&rusthome_file, io_anchored);
            let jpath = journal_path(&cli.data_dir);
            let mut journal = Journal::open(&jpath)?;
            journal.set_fsync_after_append(cli.journal_fsync);
            let mut state = replay_state(&jpath)?;

            if let Some(path) = trace_file {
                let mut trace_buf: Vec<RuleEvaluationRecord> = Vec::new();
                ingest_observation_with_causal_traced(
                    &mut journal,
                    &mut state,
                    &registry,
                    &config,
                    timestamp,
                    ObservationEvent::MotionDetected { room },
                    Uuid::new_v4(),
                    run_limits.clone(),
                    Some(&mut trace_buf),
                )?;
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)?;
                for row in trace_buf {
                    writeln!(f, "{}", serde_json::to_string(&row)?)?;
                }
            } else {
                ingest_observation(
                    &mut journal,
                    &mut state,
                    &registry,
                    &config,
                    timestamp,
                    ObservationEvent::MotionDetected { room },
                    run_limits.clone(),
                )?;
            }
            if write_snapshot {
                let digest =
                    config::resolve_rules_digest(snapshot_rules_digest.as_deref(), preset);
                save_snapshot(&cli.data_dir, &digest)?;
                println!("snapshot written");
            }
        }
        Commands::TurnOffLight {
            timestamp,
            room,
            command_id,
            causal_chain_id,
            io_anchored,
            trace_file,
            write_snapshot,
            snapshot_rules_digest,
        } => {
            let config = config::build_runtime_config(&rusthome_file, io_anchored);
            let jpath = journal_path(&cli.data_dir);
            let mut journal = Journal::open(&jpath)?;
            journal.set_fsync_after_append(cli.journal_fsync);
            let mut state = replay_state(&jpath)?;
            let command_uuid = match command_id.as_deref() {
                Some(s) => Uuid::parse_str(s).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid --command-id {s:?}: {e}"),
                    )
                })?,
                None => Uuid::new_v4(),
            };
            let causal = match causal_chain_id.as_deref() {
                Some(s) => Uuid::parse_str(s).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid --causal-chain-id {s:?}: {e}"),
                    )
                })?,
                None => Uuid::new_v4(),
            };
            if let Some(path) = trace_file {
                let mut trace_buf: Vec<RuleEvaluationRecord> = Vec::new();
                ingest_command_with_causal_traced(
                    &mut journal,
                    &mut state,
                    &registry,
                    &config,
                    timestamp,
                    CommandEvent::TurnOffLight {
                        room: room.clone(),
                        command_id: command_uuid,
                    },
                    causal,
                    run_limits.clone(),
                    Some(&mut trace_buf),
                )?;
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)?;
                for row in trace_buf {
                    writeln!(f, "{}", serde_json::to_string(&row)?)?;
                }
            } else {
                ingest_command_with_causal(
                    &mut journal,
                    &mut state,
                    &registry,
                    &config,
                    timestamp,
                    CommandEvent::TurnOffLight {
                        room,
                        command_id: command_uuid,
                    },
                    causal,
                    run_limits.clone(),
                )?;
            }
            if write_snapshot {
                let digest =
                    config::resolve_rules_digest(snapshot_rules_digest.as_deref(), preset);
                save_snapshot(&cli.data_dir, &digest)?;
                println!("snapshot written");
            }
        }
        Commands::State => {
            let st = replay_state(&journal_path(&cli.data_dir))?;
            println!("{}", serde_json::to_string_pretty(&st)?);
        }
        Commands::Replay => {
            let path = journal_path(&cli.data_dir);
            let st = replay_state(&path)?;
            let st2 = replay_state(&path)?;
            assert_eq!(st, st2);
            println!("replay_ok {}", serde_json::to_string(&st)?);
        }
        Commands::Repair { backup_suffix } => {
            let path = journal_path(&cli.data_dir);
            let (kept, dropped) = repair_journal(&path, &backup_suffix)?;
            println!("repair_ok kept={kept} dropped={dropped}");
        }
        Commands::Snapshot { rules_digest } => {
            let digest = config::resolve_rules_digest(rules_digest.as_deref(), preset);
            save_snapshot(&cli.data_dir, &digest)?;
            println!("snapshot written");
        }
        Commands::Explain { causal } => {
            let uuid = Uuid::parse_str(causal.trim())?;
            let path = journal_path(&cli.data_dir);
            let entries = load_and_sort(&path)?;
            verify_contiguous_sequence(&entries)?;
            for e in entries {
                if e.causal_chain_id == uuid {
                    println!("{}", serde_json::to_string_pretty(&e)?);
                }
            }
        }
        Commands::RulesDoc => {
            println!("```mermaid");
            println!("flowchart LR");
            for r in registry.rules() {
                for &c in r.consumes() {
                    for &p in r.produces() {
                        println!("  {:?}-->|{}|{:?}", c, r.rule_id().replace('\"', "'"), p);
                    }
                }
            }
            println!("```");
        }
        Commands::ObservedLight {
            timestamp,
            room,
            state: light,
            io_anchored,
            write_snapshot,
            snapshot_rules_digest,
        } => {
            let config = config::build_runtime_config(&rusthome_file, io_anchored);
            let jpath = journal_path(&cli.data_dir);
            let mut journal = Journal::open(&jpath)?;
            journal.set_fsync_after_append(cli.journal_fsync);
            let mut state = replay_state(&jpath)?;
            let on = matches!(light, ObservedLightState::On);
            append_observed_light_fact(
                &mut journal,
                &mut state,
                &registry,
                &config,
                ObservedLightAppend {
                    timestamp,
                    causal_chain_id: Uuid::new_v4(),
                    room,
                    on,
                    correlation_id: None,
                    trace_id: None,
                },
                run_limits.clone(),
                None,
            )?;
            if write_snapshot {
                let digest =
                    config::resolve_rules_digest(snapshot_rules_digest.as_deref(), preset);
                save_snapshot(&cli.data_dir, &digest)?;
                println!("snapshot written");
            }
        }
        Commands::Bench { count } => {
            let dir = tempfile::tempdir()?;
            let jpath = dir.path().join("events.jsonl");
            let mut config = config::build_runtime_config(&rusthome_file, false);
            config.physical_projection_mode = PhysicalProjectionMode::Simulation;
            let t0 = Instant::now();
            for i in 0..count {
                let mut journal = Journal::open(&jpath)?;
                let mut state = replay_state(&jpath)?;
                // Distinct room per iteration: otherwise `LightOn` already true → ApplyError (same growing journal).
                let room = format!("bench-{i}");
                ingest_observation(
                    &mut journal,
                    &mut state,
                    &registry,
                    &config,
                    i64::from(i),
                    ObservationEvent::MotionDetected { room },
                    run_limits.clone(),
                )?;
            }
            let ms = t0.elapsed().as_millis();
            println!("bench_emit count={count} elapsed_ms={ms}");
        }
        Commands::Init { .. } => unreachable!("handled above"),
        Commands::Serve { .. } => unreachable!("handled above"),
    }

    Ok(())
}
