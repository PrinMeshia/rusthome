//! Zigbee2MQTT → rusthome MQTT contract republisher.
//!
//! Subscribes to `zigbee2mqtt/#` (configurable), maps device state JSON to
//! `sensors/…` topics expected by [`rusthome_app::mqtt_ingest`].

mod config;
mod mapping;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::mapping::translate_z2m_publish;

#[derive(Parser, Debug)]
#[command(name = "rusthome-bridge")]
#[command(about = "Republish Zigbee2MQTT device JSON to rusthome mqtt-contract topics")]
struct Args {
    /// Path to bridge TOML (see configs/bridge.example.toml).
    #[arg(long, default_value = "configs/bridge.toml")]
    config: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();
    let cfg = Config::load(&args.config)?;

    let password = cfg.resolve_password();

    let mut mqttoptions = MqttOptions::new(
        "rusthome-bridge",
        cfg.broker.host.trim(),
        cfg.broker.port,
    );
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(30));
    if let Some(u) = &cfg.broker.username {
        mqttoptions.set_credentials(u, password.as_deref().unwrap_or_default());
    }

    let prefix = cfg.z2m.topic_prefix.trim_matches('/').to_string();
    let sub_topic = format!("{}/#", prefix);

    let (client, mut connection) = Client::new(mqttoptions, 10);
    client.subscribe(&sub_topic, QoS::AtLeastOnce)?;

    info!(
        broker = %cfg.broker.host,
        port = cfg.broker.port,
        subscribe = %sub_topic,
        devices = cfg.devices.len(),
        "rusthome-bridge connected"
    );

    for notification in connection.iter() {
        let event = match notification {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, "mqtt connection error");
                continue;
            }
        };
        let (topic, payload) = match event {
            Event::Incoming(Incoming::Publish(p)) => (p.topic, p.payload.to_vec()),
            _ => continue,
        };

        let publishes = translate_z2m_publish(&prefix, &topic, &payload, &cfg.devices);
        if publishes.is_empty() {
            debug!(topic = %topic, "no mapping or skipped");
            continue;
        }
        for p in publishes {
            match client.publish(
                p.topic.clone(),
                QoS::AtLeastOnce,
                false,
                p.payload.clone(),
            ) {
                Ok(()) => info!(to = %p.topic, from = %topic, "republished"),
                Err(e) => warn!(to = %p.topic, error = %e, "publish failed"),
            }
        }
    }

    Ok(())
}
