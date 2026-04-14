//! Read-only web dashboard — replay journal → `State`, JSON APIs.
//!
//! Used by the `rusthome-web` binary and by `rusthome serve` (CLI).

mod bluetooth_info;
mod html_pages;
mod journal;
mod security;
mod system_info;
mod util;

use std::path::{Path, PathBuf};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use rusthome_core::StateView;

use crate::html_pages::{
    bluetooth_rows_html, journal_rows_html, lights_rows_html, render_dashboard_page,
    render_system_page, system_host_rows, system_resource_rows, system_rusthome_rows,
    DASHBOARD_JOURNAL_ROWS,
};
use crate::journal::{journal_tail_dtos, JournalQuery};
use crate::security::security_banner_html;
use crate::util::esc_html;

#[derive(Clone)]
struct AppState {
    data_dir: PathBuf,
    /// Address passed to `TcpListener::bind` (shown on system page).
    listen_display: String,
}

/// Run the Axum server until SIGINT (Ctrl+C). Creates `data_dir` if missing.
pub async fn run(data_dir: PathBuf, bind: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&data_dir)?;

    let state = AppState {
        data_dir: data_dir.clone(),
        listen_display: bind.to_string(),
    };

    let app = Router::new()
        .route("/", get(page_dashboard))
        .route("/system", get(page_system))
        .route("/api/state", get(api_state))
        .route("/api/journal", get(api_journal))
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

    let journal_html = match journal_tail_dtos(&path, DASHBOARD_JOURNAL_ROWS) {
        Ok(dto) => journal_rows_html(&dto),
        Err(e) => format!(
            r#"<tr><td colspan="3" class="cell-empty error">{}</td></tr>"#,
            esc_html(&e)
        ),
    };

    let rows_html = lights_rows_html(&state);

    let last_log = state
        .last_log_item()
        .map(esc_html)
        .unwrap_or_else(|| "<em>none</em>".into());

    let html = render_dashboard_page(
        &security_banner,
        &esc_html(&path.display().to_string()),
        &rows_html,
        &journal_html,
        &last_log,
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
