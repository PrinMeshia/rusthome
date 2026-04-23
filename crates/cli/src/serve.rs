//! Embedded MQTT broker, MQTT ingest loop, and web dashboard (`rusthome serve`).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use rusthome_app::mqtt_ingest::{dispatch_mqtt_publish, wall_millis};
use rusthome_app::replay_state;
use rusthome_app::z2m_bridge_cache::{
    apply_z2m_bridge_info_payload, z2m_bridge_info_topic, Z2mBridgeCache, Z2mBridgeSnapshot,
};
use rusthome_infra::Journal;

use crate::config;

pub async fn serve_all_in_one(
    data_dir: PathBuf,
    bind: String,
    mqtt_port: u16,
    mqtt_topic: String,
    no_broker: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&data_dir)?;

    let rusthome_file = config::load_rusthome_file(&data_dir)?;
    let zigbee2mqtt = rusthome_file.zigbee2mqtt.clone();

    if no_broker {
        eprintln!("rusthome serve: broker disabled (--no-broker), web dashboard only");
        return rusthome_web::run(data_dir, &bind, None, None, zigbee2mqtt, None).await;
    }

    let (z2m_cache, z2m_bridge_info_topic): (Option<Z2mBridgeCache>, Option<String>) =
        if let Some(ref zcfg) = zigbee2mqtt {
            let t = z2m_bridge_info_topic(&zcfg.resolved_topic_prefix());
            eprintln!("rusthome serve: will subscribe to Z2M bridge info at {t}");
            let cache: Z2mBridgeCache = Arc::new(Mutex::new(Z2mBridgeSnapshot::default()));
            (Some(cache), Some(t))
        } else {
            (None, None)
        };

    let (live_tx, _) = broadcast::channel::<()>(128);

    let config = broker_config(mqtt_port);
    let mut broker = rumqttd::Broker::new(config);

    let (mut link_tx, mut link_rx) = broker
        .link("rusthome-ingest")
        .map_err(|e| format!("broker link error: {e}"))?;

    link_tx
        .subscribe(&mqtt_topic)
        .map_err(|e| format!("subscribe error: {e}"))?;
    link_tx
        .subscribe("commands/#")
        .map_err(|e| format!("subscribe commands error: {e}"))?;
    if let Some(ref t) = z2m_bridge_info_topic {
        link_tx
            .subscribe(t)
            .map_err(|e| format!("subscribe Z2M bridge/info error: {e}"))?;
    }

    let (web_link_tx, _web_link_rx) = broker
        .link("rusthome-web")
        .map_err(|e| format!("broker web link error: {e}"))?;
    let mqtt_pub = std::sync::Arc::new(std::sync::Mutex::new(web_link_tx));

    eprintln!(
        "rusthome serve: embedded MQTT broker on 0.0.0.0:{mqtt_port}, topic={mqtt_topic}, web={bind}"
    );

    let broker_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = broker.start() {
            eprintln!("broker error: {e}");
        }
    });

    let ingest_data_dir = data_dir.clone();
    let live_for_ingest = live_tx.clone();
    let z2m_for_ingest = z2m_cache.clone();
    let z2m_topic_for_ingest = z2m_bridge_info_topic.clone();
    let ingest_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = run_ingest_loop(
            &ingest_data_dir,
            &mut link_tx,
            &mut link_rx,
            Some(live_for_ingest),
            z2m_for_ingest,
            z2m_topic_for_ingest,
        ) {
            eprintln!("ingest loop error: {e}");
        }
    });

    let web_handle = tokio::spawn(async move {
        if let Err(e) = rusthome_web::run(
            data_dir,
            &bind,
            Some(mqtt_pub),
            Some(live_tx),
            zigbee2mqtt,
            z2m_cache,
        )
        .await
        {
            eprintln!("web error: {e}");
        }
    });
    let web_abort = web_handle.abort_handle();

    // `rumqttd::Broker::start` runs in `spawn_blocking` and never returns while the broker is up.
    // When the web server exits (Ctrl+C → graceful shutdown), dropping the runtime would otherwise
    // block forever waiting on those blocking threads. Exit the process so Ctrl+C always ends the CLI.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("rusthome serve: arrêt (Ctrl+C)");
            web_abort.abort();
            std::process::exit(0);
        }
        res = web_handle => {
            match res {
                Ok(()) => {}
                Err(e) => eprintln!("web task: {e}"),
            }
            std::process::exit(0);
        }
        res = broker_handle => {
            if let Err(e) = res {
                eprintln!("broker task: {e}");
            }
            std::process::exit(0);
        }
        res = ingest_handle => {
            if let Err(e) = res {
                eprintln!("ingest task: {e}");
            }
            std::process::exit(0);
        }
    }
}

fn broker_config(mqtt_port: u16) -> rumqttd::Config {
    let mut v4 = HashMap::new();
    v4.insert(
        "v4-tcp".to_string(),
        rumqttd::ServerSettings {
            name: "v4-tcp".to_string(),
            listen: SocketAddr::from(([0, 0, 0, 0], mqtt_port)),
            tls: None,
            next_connection_delay_ms: 1,
            connections: rumqttd::ConnectionSettings {
                connection_timeout_ms: 5_000,
                // Zigbee2MQTT `bridge/info` can exceed a few kB; keep headroom.
                max_payload_size: 512 * 1024,
                max_inflight_count: 100,
                auth: None,
                external_auth: None,
                dynamic_filters: true,
            },
        },
    );
    rumqttd::Config {
        id: 0,
        router: rumqttd::RouterConfig {
            max_connections: 32,
            max_outgoing_packet_count: 200,
            max_segment_size: 100 * 1024,
            max_segment_count: 10,
            custom_segment: None,
            initialized_filters: None,
            shared_subscriptions_strategy: Default::default(),
        },
        v4: Some(v4),
        v5: None,
        ws: None,
        cluster: None,
        console: None,
        bridge: None,
        prometheus: None,
        metrics: None,
    }
}

fn run_ingest_loop(
    data_dir: &Path,
    _link_tx: &mut rumqttd::local::LinkTx,
    link_rx: &mut rumqttd::local::LinkRx,
    live_tx: Option<broadcast::Sender<()>>,
    z2m_cache: Option<Z2mBridgeCache>,
    z2m_bridge_info_topic: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rusthome_file = config::load_rusthome_file(data_dir)?;
    let preset = config::resolve_rules_preset(None, &rusthome_file)?;
    let registry = preset.load_registry()?;
    let runtime_config = config::build_runtime_config(&rusthome_file, false);
    let limits = config::build_run_limits(&rusthome_file);

    let journal_path = data_dir.join("events.jsonl");
    let mut journal = Journal::open(&journal_path)?;
    let mut state = replay_state(&journal_path)?;
    let mut last_ts = wall_millis();

    let n_subs = 2 + usize::from(z2m_bridge_info_topic.is_some());
    for i in 0..n_subs {
        match link_rx.recv() {
            Ok(Some(rumqttd::Notification::DeviceAck(_))) => {
                eprintln!("ingest: subscription {}/{} acknowledged", i + 1, n_subs);
            }
            Ok(Some(other)) => {
                eprintln!("ingest: unexpected notification {}/{}: {other:?}", i + 1, n_subs);
            }
            Ok(None) => {
                eprintln!("ingest: empty notification {}/{}", i + 1, n_subs);
            }
            Err(e) => {
                return Err(format!("ingest: recv suback error: {e}").into());
            }
        }
    }

    loop {
        let notification = match link_rx.recv() {
            Ok(Some(n)) => n,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("ingest link recv error: {e}");
                break;
            }
        };

        match notification {
            rumqttd::Notification::Forward(fwd) => {
                let topic = match std::str::from_utf8(&fwd.publish.topic) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let payload = &fwd.publish.payload;

                if z2m_bridge_info_topic
                    .as_deref()
                    .is_some_and(|t| t == topic)
                {
                    if let Some(ref c) = z2m_cache {
                        if let Ok(mut g) = c.lock() {
                            if apply_z2m_bridge_info_payload(&mut g, payload) {
                                eprintln!("ingest: z2m bridge/info permit_join={:?}", g.permit_join);
                            }
                        }
                    }
                    continue;
                }

                match dispatch_mqtt_publish(
                    topic,
                    payload,
                    &mut journal,
                    &mut state,
                    &registry,
                    &runtime_config,
                    limits.clone(),
                    &mut last_ts,
                ) {
                    Ok(Some(desc)) => {
                        eprintln!("ingested {desc} topic={topic}");
                        if let Some(ref tx) = live_tx {
                            let _ = tx.send(());
                        }
                    }
                    Ok(None) => {
                        eprintln!("skip unknown topic: {topic}");
                    }
                    Err(e) => {
                        eprintln!("dispatch error on {topic}: {e}");
                    }
                }
            }
            rumqttd::Notification::Unschedule => {
                if let Err(e) = link_rx.ready() {
                    eprintln!("ingest link ready error: {e}");
                    break;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
