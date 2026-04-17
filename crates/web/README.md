# rusthome-web

Axum dashboard and JSON APIs for rusthome.

## Templates

Server-rendered HTML lives in **`templates/`** (`dashboard.html`, `sensors.html`, `system.html`). They are embedded at compile time via `include_str!` from `src/html_pages.rs` — edit the files under `templates/`, then rebuild.

## Static assets

Shared **CSS** and page **JavaScript** live in **`static/`** (`app.css`, `dashboard.js`, `sensors.js`, `system.js`). The Axum app serves them at **`/static/...`** using `include_str!` in `src/lib.rs` (no separate files needed at runtime).

Rust sources stay under **`src/`** only.
