//! Subscribe to an **external** MQTT broker and ingest MQTT publishes (adapter template).
//!
//! Uses [`rusthome_app::integrations::mqtt::dispatch_mqtt_publish`] for both observations and commands:
//! - `sensors/motion/{room}` → `MotionDetected`
//! - `sensors/temperature/{sensor_id}` → `TemperatureReading`
//! - `sensors/contact/{sensor_id}` → `ContactChanged`
//! - `commands/light/{room}/on|off` → `TurnOnLight` / `TurnOffLight`
//!
//! One process handles one `--topic` filter; subscribe to `sensors/#` for sensors, or run a
//! second instance with `--topic 'commands/#'` for light commands (or extend this example with
//! multiple subscriptions).
//!
//! ```text
//! cargo run -p rusthome-app --example mqtt_motion_ingest -- \
//!   --data-dir data --broker 127.0.0.1 --port 1883 --topic 'sensors/#'
//! ```

use std::path::PathBuf;

use clap::Parser;
use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use rusthome_app::integrations::mqtt::{dispatch_mqtt_publish, wall_millis};
use rusthome_app::{replay_state, rusthome_file};
use rusthome_infra::Journal;

#[derive(Parser, Debug)]
#[command(about = "MQTT → observation adapter (connects to external broker)")]
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

    let mut mqttoptions = MqttOptions::new("rusthome-mqtt-ingest", &args.broker, args.port);
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(30));
    if let (Some(u), Some(p)) = (&args.mqtt_user, &args.mqtt_password) {
        mqttoptions.set_credentials(u, p);
    }

    let (client, mut connection) = Client::new(mqttoptions, 10);
    client.subscribe(&args.topic, QoS::AtLeastOnce)?;

    let mut last_ts = wall_millis();
    eprintln!(
        "mqtt: data_dir={} broker={}:{} topic={}",
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

        match dispatch_mqtt_publish(
            &topic,
            &payload,
            &mut journal,
            &mut state,
            &registry,
            &config,
            limits.clone(),
            &mut last_ts,
        ) {
            Ok(Some(desc)) => eprintln!("ingested {desc} topic={topic}"),
            Ok(None) => eprintln!("skip unknown topic: {topic}"),
            Err(e) => eprintln!("dispatch error on {topic}: {e}"),
        }
    }

    Ok(())
}
