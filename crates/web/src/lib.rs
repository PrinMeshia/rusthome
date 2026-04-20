//! Web dashboard — replay journal → `State`, JSON APIs, command publishing.
//!
//! Used by the `rusthome-web` binary and by `rusthome serve` (CLI).

mod bluetooth_info;
mod html_pages;
mod journal;
mod security;
mod sensor_display;
mod system_info;
mod util;

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::StreamExt as _;
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use serde::Deserialize;
use rusthome_app::rusthome_file::Zigbee2MqttConfig;

use crate::html_pages::{
    bluetooth_rows_html, contact_rows_html, journal_rows_html, lights_rows_html,
    humidity_rows_html, render_dashboard_page, render_sensors_page, render_system_page,
    sensors_rows_html, summary_cards_html, system_host_rows, system_resource_rows,
    system_rusthome_rows, system_serial_rows_html, temperature_rows_html, zigbee2mqtt_panel_html,
    DASHBOARD_JOURNAL_ROWS,
};
use crate::journal::{journal_tail_dtos, JournalQuery};
use crate::security::security_banner_html;
use crate::sensor_display::{
    load_or_default as sensor_display_load, merge_from_state as sensor_display_merge,
    save as sensor_display_save, sensor_display_path, validate_document as sensor_display_validate,
    SensorDisplay,
};
use crate::util::esc_html;

/// Opaque handle for publishing MQTT messages to the embedded broker.
pub type MqttPub = Arc<Mutex<rumqttd::local::LinkTx>>;

#[derive(Clone)]
struct AppState {
    data_dir: PathBuf,
    /// Address passed to `TcpListener::bind` (shown on system page).
    listen_display: String,
    /// When running with the embedded broker, allows publishing commands.
    mqtt_pub: Option<MqttPub>,
    /// When set (`rusthome serve` with broker), ingest notifies on each journal update → `/api/live` SSE.
    live_tx: Option<broadcast::Sender<()>>,
    /// Optional `[zigbee2mqtt]` from `rusthome.toml` (permit join via MQTT).
    zigbee2mqtt: Option<Zigbee2MqttConfig>,
}

/// Run the Axum server until SIGINT (Ctrl+C). Creates `data_dir` if missing.
///
/// Pass `mqtt_pub` when running under `rusthome serve` with the embedded broker.
/// Pass `None` for standalone / `--no-broker` mode (command endpoint returns 503).
///
/// Pass `live_events` when ingest runs in-process (`rusthome serve` with broker): each journal
/// update from MQTT dispatch notifies `GET /api/live` (SSE) so the dashboard can refresh immediately.
///
/// Pass `zigbee2mqtt` from `{data-dir}/rusthome.toml` to enable Zigbee2MQTT bridge helpers (e.g. permit join).
pub async fn run(
    data_dir: PathBuf,
    bind: &str,
    mqtt_pub: Option<MqttPub>,
    live_events: Option<broadcast::Sender<()>>,
    zigbee2mqtt: Option<Zigbee2MqttConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&data_dir)?;

    let state = AppState {
        data_dir: data_dir.clone(),
        listen_display: bind.to_string(),
        mqtt_pub,
        live_tx: live_events,
        zigbee2mqtt,
    };

    let app = Router::new()
        .route("/static/app.css", get(serve_app_css))
        .route("/static/common.js", get(serve_common_js))
        .route("/static/dashboard.js", get(serve_dashboard_js))
        .route("/static/sensors.js", get(serve_sensors_js))
        .route("/static/system.js", get(serve_system_js))
        .route(
            "/docs/mqtt-contract",
            get(serve_mqtt_contract_markdown),
        )
        .route("/", get(page_dashboard))
        .route("/sensors", get(page_sensors))
        .route("/system", get(page_system))
        .route("/api/state", get(api_state))
        .route(
            "/api/sensor-display",
            get(api_sensor_display_get).put(api_sensor_display_put),
        )
        .route(
            "/api/sensor-display/sync-from-state",
            post(api_sensor_display_sync),
        )
        .route("/api/journal", get(api_journal))
        .route("/api/live", get(api_live_sse))
        .route("/api/command", post(api_command))
        .route("/api/observation", post(api_observation))
        .route("/api/system", get(api_system))
        .route("/api/bluetooth", get(api_bluetooth))
        .route("/api/bluetooth/device", get(api_bluetooth_device))
        .route("/api/bluetooth/info", get(api_bluetooth_info))
        .route(
            "/api/zigbee2mqtt/permit_join",
            post(api_zigbee2mqtt_permit_join),
        )
        .route("/api/health", get(api_health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!(
        "rusthome web UI listening on http://{} (data-dir={})",
        bind,
        data_dir.display()
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn api_live_sse(State(st): State<AppState>) -> impl IntoResponse {
    let Some(live_tx) = st.live_tx.clone() else {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Flux temps réel indisponible (lancez rusthome serve avec le broker MQTT intégré).\n",
        )
            .into_response();
    };

    let rx = live_tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|item| {
        match item {
            Ok(()) => (),
            Err(BroadcastStreamRecvError::Lagged(_)) => (),
        }
        Ok::<Event, Infallible>(Event::default().data("{}"))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(25))
            .text("ping"),
    )
    .into_response()
}

async fn serve_app_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/app.css"),
    )
}

async fn serve_common_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../static/common.js"),
    )
}

async fn serve_dashboard_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../static/dashboard.js"),
    )
}

async fn serve_sensors_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../static/sensors.js"),
    )
}

async fn serve_mqtt_contract_markdown() -> impl IntoResponse {
    const MD: &str = include_str!("../../../docs/mqtt-contract.md");
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (
                header::CACHE_CONTROL,
                "public, max-age=3600",
            ),
        ],
        MD,
    )
}

async fn serve_system_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../static/system.js"),
    )
}

fn journal_path(data_dir: &Path) -> PathBuf {
    data_dir.join("events.jsonl")
}

async fn page_dashboard(State(st): State<AppState>) -> impl IntoResponse {
    let security_banner = security_banner_html(&st.listen_display);
    let path = journal_path(&st.data_dir);
    let state = match rusthome_app::replay_state(&path) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    "<!DOCTYPE html><html><body><h1>replay error</h1><pre>{}</pre></body></html>",
                    esc_html(&e.to_string())
                )),
            )
                .into_response();
        }
    };

    let (journal_html, journal_count) = match journal_tail_dtos(&path, DASHBOARD_JOURNAL_ROWS) {
        Ok(dto) => {
            let html = journal_rows_html(&dto);
            let count = dto.last().map(|d| d.sequence + 1).unwrap_or(0) as usize;
            (html, count)
        }
        Err(e) => (
            format!(
                r#"<tr><td colspan="3" class="cell-empty error">{}</td></tr>"#,
                esc_html(&e)
            ),
            0,
        ),
    };

    let broker_available = st.mqtt_pub.is_some();
    let rows_html = lights_rows_html(&state, broker_available);
    let summary = summary_cards_html(&state, journal_count);
    let sensors = sensors_rows_html(&state);

    let html = render_dashboard_page(
        &security_banner,
        &esc_html(&path.display().to_string()),
        &rows_html,
        &journal_html,
        &summary,
        &sensors,
        broker_available,
        st.live_tx.is_some(),
    );
    Html(html).into_response()
}

async fn page_sensors(State(st): State<AppState>) -> impl IntoResponse {
    let security_banner = security_banner_html(&st.listen_display);
    let path = journal_path(&st.data_dir);
    let state = match rusthome_app::replay_state(&path) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    "<!DOCTYPE html><html><body><h1>replay error</h1><pre>{}</pre></body></html>",
                    esc_html(&e.to_string())
                )),
            )
                .into_response();
        }
    };

    let broker_available = st.mqtt_pub.is_some();
    let html = render_sensors_page(
        &security_banner,
        &temperature_rows_html(&state),
        &humidity_rows_html(&state),
        &contact_rows_html(&state),
        broker_available,
        st.live_tx.is_some(),
    );
    Html(html).into_response()
}

async fn api_state(State(st): State<AppState>) -> impl IntoResponse {
    let path = journal_path(&st.data_dir);
    match rusthome_app::replay_state(&path) {
        Ok(s) => Json(s).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("replay error: {e}"),
        )
            .into_response(),
    }
}

async fn api_sensor_display_get(State(st): State<AppState>) -> impl IntoResponse {
    let path = sensor_display_path(&st.data_dir);
    match sensor_display_load(&path) {
        Ok(d) => Json(d).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("sensor_display load: {e}"),
        )
            .into_response(),
    }
}

async fn api_sensor_display_put(
    State(st): State<AppState>,
    Json(body): Json<SensorDisplay>,
) -> impl IntoResponse {
    if let Err(msg) = sensor_display_validate(&body) {
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }
    let path = sensor_display_path(&st.data_dir);
    match sensor_display_save(&path, &body) {
        Ok(()) => Json(body).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("sensor_display save: {e}"),
        )
            .into_response(),
    }
}

async fn api_sensor_display_sync(State(st): State<AppState>) -> impl IntoResponse {
    let journal = journal_path(&st.data_dir);
    let state = match rusthome_app::replay_state(&journal) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("replay error: {e}"),
            )
                .into_response();
        }
    };
    let path = sensor_display_path(&st.data_dir);
    let mut d = match sensor_display_load(&path) {
        Ok(doc) => doc,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("sensor_display load: {e}"),
            )
                .into_response();
        }
    };
    sensor_display_merge(&state, &mut d);
    if let Err(msg) = sensor_display_validate(&d) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("sensor_display validate after merge: {msg}"),
        )
            .into_response();
    }
    match sensor_display_save(&path, &d) {
        Ok(()) => Json(d).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("sensor_display save: {e}"),
        )
            .into_response(),
    }
}

async fn api_journal(
    State(st): State<AppState>,
    Query(q): Query<JournalQuery>,
) -> impl IntoResponse {
    let path = journal_path(&st.data_dir);
    match journal_tail_dtos(&path, q.limit) {
        Ok(dto) => Json(dto).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("journal load: {e}"),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CommandRequest {
    action: String,
    room: String,
}

#[derive(Deserialize)]
struct ObservationRequest {
    /// `motion` | `temperature` | `humidity` | `contact`
    kind: String,
    /// Last segment of the MQTT topic (`sensors/<kind>/<entity>`).
    entity: String,
    /// Motion only: optional JSON `room` override (otherwise the topic entity is used).
    #[serde(default)]
    room: Option<String>,
    #[serde(default)]
    celsius: Option<f64>,
    #[serde(default)]
    millidegrees_c: Option<i32>,
    #[serde(default)]
    percent_rh: Option<f64>,
    #[serde(default)]
    permille_rh: Option<i32>,
    #[serde(default)]
    open: Option<bool>,
}

fn validate_topic_entity(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Err("identifiant topic requis".into());
    }
    if t.len() > 128 {
        return Err("identifiant trop long (max 128)".into());
    }
    if t.chars()
        .any(|c| c == '/' || c == '+' || c == '#' || c.is_whitespace())
    {
        return Err("l'identifiant ne doit pas contenir d'espace ni /, + ou #".into());
    }
    Ok(t.to_string())
}

fn normalize_motion_room(room: &Option<String>) -> Result<Option<String>, String> {
    let Some(ref r) = room else {
        return Ok(None);
    };
    let t = r.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.len() > 128 {
        return Err("nom de pièce trop long (max 128)".into());
    }
    if t.chars().any(|c| c == '\n' || c == '\r') {
        return Err("nom de pièce invalide".into());
    }
    Ok(Some(t.to_string()))
}

fn observation_topic_and_payload(req: &ObservationRequest) -> Result<(String, Vec<u8>), String> {
    let kind = req.kind.trim().to_lowercase();
    let entity = validate_topic_entity(&req.entity)?;
    match kind.as_str() {
        "motion" => {
            let topic = format!("sensors/motion/{entity}");
            let room = normalize_motion_room(&req.room)?;
            let payload = if let Some(r) = room {
                serde_json::to_vec(&serde_json::json!({ "room": r })).map_err(|e| e.to_string())?
            } else {
                Vec::new()
            };
            Ok((topic, payload))
        }
        "temperature" => {
            let millideg = if let Some(md) = req.millidegrees_c {
                md
            } else if let Some(c) = req.celsius {
                (c * 1000.0).round() as i32
            } else {
                return Err("température : renseignez celsius ou millidegrees_c".into());
            };
            if !(-100_000..=200_000).contains(&millideg) {
                return Err("température hors plage plausible".into());
            }
            let topic = format!("sensors/temperature/{entity}");
            let payload = serde_json::to_vec(&serde_json::json!({ "millidegrees_c": millideg }))
                .map_err(|e| e.to_string())?;
            Ok((topic, payload))
        }
        "humidity" => {
            let permille: i32 = if let Some(p) = req.permille_rh {
                p.clamp(0, 1000)
            } else if let Some(pct) = req.percent_rh {
                if !(-0.001..=100.001).contains(&pct) {
                    return Err("humidité relative : 0 à 100 %".into());
                }
                ((pct * 10.0).round() as i32).clamp(0, 1000)
            } else {
                return Err("humidité : renseignez percent_rh (0–100) ou permille_rh (0–1000)".into());
            };
            let topic = format!("sensors/humidity/{entity}");
            let payload = serde_json::to_vec(&serde_json::json!({ "permille_rh": permille }))
                .map_err(|e| e.to_string())?;
            Ok((topic, payload))
        }
        "contact" => {
            let open = req
                .open
                .ok_or_else(|| "contact : indiquez open (true ou false)".to_string())?;
            let topic = format!("sensors/contact/{entity}");
            let payload =
                serde_json::to_vec(&serde_json::json!({ "open": open })).map_err(|e| e.to_string())?;
            Ok((topic, payload))
        }
        _ => Err("type inconnu : motion, temperature, humidity ou contact".into()),
    }
}

async fn api_observation(
    State(st): State<AppState>,
    Json(req): Json<ObservationRequest>,
) -> impl IntoResponse {
    let Some(pub_handle) = &st.mqtt_pub else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "broker not available (--no-broker mode)",
        )
            .into_response();
    };
    let (topic, payload) = match observation_topic_and_payload(&req) {
        Ok(x) => x,
        Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
    };
    let mut tx = pub_handle.lock().unwrap();
    match tx.publish(topic, bytes::Bytes::from(payload)) {
        Ok(_) => (
            StatusCode::ACCEPTED,
            "observation published",
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("publish error: {e}"),
        )
            .into_response(),
    }
}

#[derive(Deserialize, Default)]
struct Zigbee2MqttPermitJoinBody {
    #[serde(default)]
    seconds: Option<u64>,
}

async fn api_zigbee2mqtt_permit_join(
    State(st): State<AppState>,
    Json(body): Json<Zigbee2MqttPermitJoinBody>,
) -> impl IntoResponse {
    let Some(ref zcfg) = st.zigbee2mqtt else {
        return (
            StatusCode::NOT_FOUND,
            "zigbee2mqtt not configured: add [zigbee2mqtt] to rusthome.toml\n",
        )
            .into_response();
    };
    let Some(pub_handle) = &st.mqtt_pub else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "broker not available (--no-broker mode)\n",
        )
            .into_response();
    };
    let secs = body
        .seconds
        .unwrap_or_else(|| zcfg.resolved_permit_join_seconds())
        .clamp(1, 900);
    let prefix = zcfg.resolved_topic_prefix();
    let topic = format!(
        "{}/bridge/request",
        prefix.trim_end_matches('/').trim_start_matches('/')
    );
    let payload = serde_json::json!({
        "type": "permit_join",
        "value": true,
        "time": secs,
    });
    let bytes = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("json error: {e}"),
            )
                .into_response();
        }
    };
    let mut tx = pub_handle.lock().unwrap();
    match tx.publish(topic, bytes::Bytes::from(bytes)) {
        Ok(_) => (
            StatusCode::ACCEPTED,
            format!("permit join published ({secs} s)\n"),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("publish error: {e}\n"),
        )
            .into_response(),
    }
}

async fn api_command(
    State(st): State<AppState>,
    Json(req): Json<CommandRequest>,
) -> impl IntoResponse {
    let Some(pub_handle) = &st.mqtt_pub else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "broker not available (--no-broker mode)",
        )
            .into_response();
    };
    let topic = match req.action.as_str() {
        "turn_on" => format!("commands/light/{}/on", req.room),
        "turn_off" => format!("commands/light/{}/off", req.room),
        _ => {
            return (StatusCode::BAD_REQUEST, "unknown action").into_response();
        }
    };
    let mut tx = pub_handle.lock().unwrap();
    match tx.publish(topic, bytes::Bytes::new()) {
        Ok(_) => (StatusCode::ACCEPTED, "command published").into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("publish error: {e}"),
        )
            .into_response(),
    }
}

async fn page_system(State(st): State<AppState>) -> impl IntoResponse {
    let st = st.clone();
    let (snap, bt) = match tokio::task::spawn_blocking(move || {
        let jp = journal_path(&st.data_dir);
        (
            system_info::capture(&st.listen_display, &st.data_dir, &jp),
            bluetooth_info::snapshot(),
        )
    })
    .await
    {
        Ok(pair) => pair,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "system snapshot task failed",
            )
                .into_response();
        }
    };
    let security_banner = security_banner_html(&snap.listen);
    let serial_rows = system_serial_rows_html(&snap.serial_ports);
    let z2m_panel = st
        .zigbee2mqtt
        .as_ref()
        .map(|z| zigbee2mqtt_panel_html(z, st.mqtt_pub.is_some()))
        .unwrap_or_default();
    let html = render_system_page(
        &security_banner,
        &system_rusthome_rows(&snap),
        &system_host_rows(&snap),
        &system_resource_rows(&snap),
        &serial_rows,
        &z2m_panel,
        &bluetooth_rows_html(&bt),
    );
    Html(html).into_response()
}

async fn api_system(State(st): State<AppState>) -> impl IntoResponse {
    let st = st.clone();
    match tokio::task::spawn_blocking(move || {
        let jp = journal_path(&st.data_dir);
        system_info::capture(&st.listen_display, &st.data_dir, &jp)
    })
    .await
    {
        Ok(snap) => Json(snap).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "system snapshot task failed",
        )
            .into_response(),
    }
}

async fn api_bluetooth() -> impl IntoResponse {
    match tokio::task::spawn_blocking(bluetooth_info::snapshot).await {
        Ok(snap) => Json(snap).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "bluetooth snapshot task failed",
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct BluetoothDeviceQuery {
    /// MAC as typed by the user (normalized server-side).
    addr: String,
    /// Optional discovery duration in seconds (5–45). Runs `bluetoothctl scan on` before listing devices.
    #[serde(default)]
    scan: Option<u32>,
}

#[derive(Deserialize)]
struct BluetoothAddrOnlyQuery {
    addr: String,
}

async fn api_bluetooth_info(Query(q): Query<BluetoothAddrOnlyQuery>) -> impl IntoResponse {
    if q.addr.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Paramètre requis : addr (adresse MAC).\n",
        )
            .into_response();
    }
    match tokio::task::spawn_blocking(move || bluetooth_info::device_info(&q.addr)).await {
        Ok(info) => Json(info).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "bluetooth info task failed",
        )
            .into_response(),
    }
}

async fn api_bluetooth_device(Query(q): Query<BluetoothDeviceQuery>) -> impl IntoResponse {
    if q.addr.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Paramètre requis : addr (adresse MAC).\n",
        )
            .into_response();
    }
    match tokio::task::spawn_blocking(move || bluetooth_info::lookup_device(&q.addr, q.scan)).await {
        Ok(lookup) => Json(lookup).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "bluetooth device lookup failed",
        )
            .into_response(),
    }
}

async fn api_health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true, "service": "rusthome-web" }))
}
