# rusthome-web

Axum dashboard and JSON APIs for rusthome.

## Templates

Server-rendered HTML lives in **`templates/`** (`dashboard.html`, `sensors.html`, `system.html`). They are embedded at compile time via `include_str!` from `src/html_pages.rs` — edit the files under `templates/`, then rebuild.

Rust sources stay under **`src/`** only.
