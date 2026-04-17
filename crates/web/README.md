# rusthome-web

Axum dashboard and JSON APIs for rusthome.

## Templates

Server-rendered HTML lives in **`templates/`** (`dashboard.html`, `sensors.html`, `system.html`). They are embedded at compile time via `include_str!` from `src/html_pages.rs` — edit the files under `templates/`, then rebuild.

## Static assets

Shared **CSS** and page **JavaScript** live in **`static/`** (`app.css`, `common.js`, `dashboard.js`, `sensors.js`, `system.js`). The Axum app serves them at **`/static/...`** using `include_str!` in `src/lib.rs` (no separate files needed at runtime).

- **`common.js`** — UI preferences in `localStorage` (no account): **`rusthome-theme`** (`system` default, or `light` / `dark`), **`rusthome-density`** (`compact` or unset for confort), **`rusthome-refresh-ms`** (`0` = manual only, or `2000` / `4000` / `10000`), **`rusthome-journal-limit`** on the dashboard only (`20` / `40` / `80` / `120`, default follows server config). Scripts load with **`defer`**; `initPrefs()` wires `#theme-select`, `#density-select`, `#refresh-interval-select`, and `#journal-limit-select` when present. **`window.rhSetTheme`**, **`window.rhSetDensity`**, **`window.rhGetRefreshIntervalMs`**, and **`window.rhGetJournalLimit(default)`** are available for the console or other scripts. Changing the refresh interval dispatches **`rh-refresh-interval-changed`**; changing the journal limit dispatches **`rh-journal-limit-changed`**.

## UI structure

Pages use a shared **sticky shell**: brand + main nav (`%%MAIN_NAV%%` from `html_pages.rs`), toolbar (refresh, auto-refresh interval, optional journal row count on the dashboard, density, theme), then content. API doc links are grouped in a **developer footer** (`%%DEV_FOOTER%%`). Styling relies on **CSS custom properties** (tokens) in `app.css` for spacing, radii, shadows, typography, and focus rings, with `html[data-theme="dark"|"light"]` overriding media-based defaults when the user picks a manual theme, and **`html[data-density="compact"]`** for a tighter layout.

## Live updates (SSE)

With **`rusthome serve`** and the **embedded MQTT broker**, each successful MQTT ingest that appends to the journal notifies a broadcast channel. **`GET /api/live`** exposes a **Server-Sent Events** stream (JSON payload `{}` per update, plus periodic keep-alives). The dashboard loads with **`livePush: true`** in `#rh-dashboard-config` and opens an **`EventSource`** to `/api/live`, then **debounces** (~80 ms) and calls the same **`refresh()`** as polling — so a motion sensor published over MQTT can update the UI almost immediately.

This path is **not** available for standalone **`rusthome-web`** or **`rusthome serve --no-broker`** (no in-process ingest). External writes to the journal (e.g. another `rusthome emit` process) are not pushed until the next poll or page reload.

Rust sources stay under **`src/`** only.
