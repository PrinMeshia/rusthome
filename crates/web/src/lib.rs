//! Web dashboard — replay journal → `State`, JSON APIs, command publishing.
//!
//! Used by the `rusthome-web` binary and by `rusthome serve` (CLI).

mod bluetooth_info;
mod html_pages;
mod journal;
mod security;
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

use crate::html_pages::{
    bluetooth_rows_html, contact_rows_html, journal_rows_html, lights_rows_html,
    render_dashboard_page, render_sensors_page, render_system_page, sensors_rows_html,
    summary_cards_html, system_host_rows, system_resource_rows, system_rusthome_rows,
    temperature_rows_html, DASHBOARD_JOURNAL_ROWS,
};
use crate::journal::{journal_tail_dtos, JournalQuery};
use crate::security::security_banner_html;
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
}

/// Run the Axum server until SIGINT (Ctrl+C). Creates `data_dir` if missing.
///
/// Pass `mqtt_pub` when running under `rusthome serve` with the embedded broker.
/// Pass `None` for standalone / `--no-broker` mode (command endpoint returns 503).
///
/// Pass `live_events` when ingest runs in-process (`rusthome serve` with broker): each journal
/// update from MQTT dispatch notifies `GET /api/live` (SSE) so the dashboard can refresh immediately.
pub async fn run(
    data_dir: PathBuf,
    bind: &str,
    mqtt_pub: Option<MqttPub>,
    live_events: Option<broadcast::Sender<()>>,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&data_dir)?;

    let state = AppState {
        data_dir: data_dir.clone(),
        listen_display: bind.to_string(),
        mqtt_pub,
        live_tx: live_events,
    };

    let app = Router::new()
        .route("/static/app.css", get(serve_app_css))
        .route("/static/common.js", get(serve_common_js))
        .route("/static/dashboard.js", get(serve_dashboard_js))
        .route("/static/sensors.js", get(serve_sensors_js))
        .route("/static/system.js", get(serve_system_js))
        .route("/", get(page_dashboard))
        .route("/sensors", get(page_sensors))
        .route("/system", get(page_system))
        .route("/api/state", get(api_state))
        .route("/api/journal", get(api_journal))
        .route("/api/live", get(api_live_sse))
        .route("/api/command", post(api_command))
        .route("/api/system", get(api_system))
        .route("/api/bluetooth", get(api_bluetooth))
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

    let html = render_sensors_page(
        &security_banner,
        &temperature_rows_html(&state),
        &contact_rows_html(&state),
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
    let html = render_system_page(
        &security_banner,
        &system_rusthome_rows(&snap),
        &system_host_rows(&snap),
        &system_resource_rows(&snap),
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

async fn api_health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true, "service": "rusthome-web" }))
}
