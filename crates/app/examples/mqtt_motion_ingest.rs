//! Subscribe to MQTT and append **`MotionDetected`** observations (adapter template).
//!
//! - **Payload**: UTF-8 room name, or JSON `{"room":"hall"}`; optional `"ts": <i64>` for logical
//!   timestamp (otherwise uses wall-clock ms, bumped to stay strictly increasing vs the last append).
//! - Loads `{data-dir}/rusthome.toml` like other examples (`--rules-preset` / `--io-anchored` override).
//!
//! Depends on **`rumqttc`** with **`default-features = false`** (plain TCP MQTT, no bundled rustls).
//!
//! ```text
//! cargo run -p rusthome-app --example mqtt_motion_ingest -- \
//!   --data-dir data --broker 127.0.0.1 --port 1883 --topic 'sensors/motion/#'
//! ```
//!
//! Test publish (Mosquitto):
//! `mosquitto_pub -t sensors/motion/hall -m hall` or `-m '{"room":"kitchen"}'`

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use rusthome_app::{ingest_observation_with_causal, replay_state, rusthome_file};
use rusthome_core::ObservationEvent;
use rusthome_infra::Journal;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(about = "MQTT → MotionDetected adapter (library demo)")]
struct Args {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    broker: String,
    #[arg(long, default_value_t = 1883)]
    port: u16,
    #[arg(long)]
    topic: String,
    #[arg(long = "rules-preset")]
    rules_preset: Option<String>,
    #[arg(long, default_value_t = false)]
    io_anchored: bool,
    #[arg(long)]
    mqtt_user: Option<String>,
    #[arg(long)]
    mqtt_password: Option<String>,
}

fn wall_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn room_from_payload_and_topic(payload: &[u8], topic: &str) -> Result<String, String> {
    let s = std::str::from_utf8(payload).map_err(|_| "payload is not UTF-8".to_string())?;
    let s = s.trim();
    if s.is_empty() {
        return Err("empty payload".into());
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        if let Some(r) = v.get("room").and_then(|x| x.as_str()) {
            return Ok(r.to_string());
        }
    }
    if !s.contains('{') && s.len() < 256 {
        return Ok(s.to_string());
    }
    let seg = topic.rsplit('/').next().filter(|x| !x.is_empty() && *x != "#" && *x != "+");
    seg.map(String::from).ok_or_else(|| {
        "could not parse room from JSON payload or topic tail; use {\"room\":\"…\"} or plain name"
            .into()
    })
}

fn optional_ts_from_payload(payload: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(payload).ok()?.trim();
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    v.get("ts")?.as_i64()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let file = rusthome_file::load_rusthome_file(&args.data_dir)?;
    let preset = rusthome_file::resolve_rules_preset(args.rules_preset.as_deref(), &file)?;
    let registry = preset.load_registry()?;
    let config = rusthome_file::build_runtime_config(&file, args.io_anchored);
    let limits = rusthome_file::build_run_limits(&file);

    let journal_path = args.data_dir.join("events.jsonl");
    let mut journal = Journal::open(&journal_path)?;
    let mut state = replay_state(&journal_path)?;

    let mut mqttoptions = MqttOptions::new("rusthome-mqtt-motion", &args.broker, args.port);
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(30));
    if let (Some(u), Some(p)) = (&args.mqtt_user, &args.mqtt_password) {
        mqttoptions.set_credentials(u, p);
    }

    let (client, mut connection) = Client::new(mqttoptions, 10);
    client.subscribe(&args.topic, QoS::AtLeastOnce)?;

    let last_ts = RefCell::new(0i64);
    eprintln!(
        "mqtt_motion_ingest: data_dir={} broker={}:{} topic={}",
        args.data_dir.display(),
        args.broker,
        args.port,
        args.topic
    );

    for notification in connection.iter() {
        let event = match notification {
            Ok(e) => e,
            Err(e) => {
                eprintln!("mqtt connection error: {e}");
                continue;
            }
        };
        let (topic, payload) = match event {
            Event::Incoming(Incoming::Publish(p)) => (p.topic, p.payload.to_vec()),
            _ => continue,
        };

        let room = match room_from_payload_and_topic(&payload, &topic) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skip publish on {topic}: {e}");
                continue;
            }
        };

        let candidate = optional_ts_from_payload(&payload).unwrap_or_else(wall_millis);
        let ts = {
            let mut t = last_ts.borrow_mut();
            let n = (*t + 1).max(candidate);
            *t = n;
            n
        };

        let causal = Uuid::new_v4();
        match ingest_observation_with_causal(
            &mut journal,
            &mut state,
            &registry,
            &config,
            ts,
            ObservationEvent::MotionDetected { room: room.clone() },
            causal,
            limits.clone(),
        ) {
            Ok(()) => eprintln!("ingested MotionDetected room={room} ts={ts} topic={topic}"),
            Err(e) => eprintln!("ingest error room={room}: {e:?}"),
        }
    }

    Ok(())
}
