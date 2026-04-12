//! Read-only web dashboard — replay journal → `State`, JSON APIs.
//!
//! Used by the `rusthome-web` binary and by `rusthome serve` (CLI).

use std::path::{Path, PathBuf};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use rusthome_core::{EventKind, StateView};
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone)]
struct AppState {
    data_dir: PathBuf,
}

/// Run the Axum server until SIGINT (Ctrl+C). Creates `data_dir` if missing.
pub async fn run(data_dir: PathBuf, bind: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&data_dir)?;

    let state = AppState {
        data_dir: data_dir.clone(),
    };

    let app = Router::new()
        .route("/", get(page_dashboard))
        .route("/api/state", get(api_state))
        .route("/api/journal", get(api_journal))
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

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn page_dashboard(State(st): State<AppState>) -> impl IntoResponse {
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

    let mut rows_html = String::new();
    let rows = state.light_room_rows();
    if rows.is_empty() {
        rows_html.push_str("<tr><td colspan=\"3\"><em>No rooms in projection yet</em></td></tr>");
    } else {
        for (room, on, prov) in rows {
            let p = prov
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "—".to_string());
            rows_html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc_html(&room),
                if on { "on" } else { "off" },
                esc_html(&p),
            ));
        }
    }

    let last_log = state
        .last_log_item()
        .map(esc_html)
        .unwrap_or_else(|| "<em>none</em>".into());

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>rusthome</title>
  <style>
    :root {{ color-scheme: light dark; }}
    body {{ font-family: system-ui, sans-serif; max-width: 56rem; margin: 0 auto; padding: 1rem 1.25rem; line-height: 1.45; }}
    h1 {{ font-size: 1.35rem; margin-top: 0; }}
    nav {{ margin: 0.75rem 0 1.25rem; }}
    nav a {{ margin-right: 1rem; }}
    table {{ border-collapse: collapse; width: 100%; margin: 1rem 0; }}
    th, td {{ border: 1px solid #8884; padding: 0.45rem 0.65rem; text-align: left; }}
    th {{ font-weight: 600; }}
    .meta {{ font-size: 0.9rem; opacity: 0.85; }}
    code {{ font-size: 0.88rem; }}
  </style>
</head>
<body>
  <h1>rusthome — projection</h1>
  <p class="meta">Read-only replay of <code>{}</code></p>
  <nav><a href="/api/state">JSON state</a> · <a href="/api/journal?limit=40">JSON journal</a> · <a href="/api/health">health</a></nav>
  <h2>Lights</h2>
  <table>
    <thead><tr><th>Room</th><th>State</th><th>Last provenance</th></tr></thead>
    <tbody>{}</tbody>
  </table>
  <h2>Usage log (demo)</h2>
  <p>{}</p>
</body>
</html>"#,
        esc_html(&path.display().to_string()),
        rows_html,
        last_log,
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

#[derive(Deserialize)]
struct JournalQuery {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    40
}

#[derive(Serialize)]
struct JournalLineDto {
    sequence: u64,
    timestamp: i64,
    kind: EventKind,
}

async fn api_journal(
    State(st): State<AppState>,
    Query(q): Query<JournalQuery>,
) -> impl IntoResponse {
    let path = journal_path(&st.data_dir);
    let entries = match rusthome_infra::load_and_sort(&path) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("journal load: {e}"),
            )
                .into_response();
        }
    };
    let lim = q.limit.clamp(1, 500);
    let tail = if entries.len() > lim {
        let start = entries.len() - lim;
        let mut v = entries;
        v.split_off(start)
    } else {
        entries
    };
    let dto: Vec<JournalLineDto> = tail
        .into_iter()
        .map(|e| JournalLineDto {
            sequence: e.sequence,
            timestamp: e.timestamp,
            kind: e.event.kind(),
        })
        .collect();
    Json(dto).into_response()
}

async fn api_health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true, "service": "rusthome-web" }))
}
