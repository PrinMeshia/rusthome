//! Server-side HTML fragments and full page templates (`include_str!`).

use rusthome_core::{EventKind, State as CoreState};

use crate::bluetooth_info;
use crate::journal::JournalLineDto;
use crate::system_info;
use crate::util::esc_html;

/// Tail length for the dashboard journal panel (SSR + JS refresh).
pub(crate) const DASHBOARD_JOURNAL_ROWS: usize = 40;

fn event_kind_snake(k: &EventKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{k:?}"))
}

pub(crate) fn lights_rows_html(state: &CoreState) -> String {
    let mut rows_html = String::new();
    let rows = state.light_room_rows();
    if rows.is_empty() {
        rows_html.push_str(
            r#"<tr><td colspan="3" class="cell-empty"><em>No rooms in projection yet</em></td></tr>"#,
        );
    } else {
        for (room, on, prov) in rows {
            let p = prov
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "—".to_string());
            let badge_class = if on { "on" } else { "off" };
            let badge_text = if on { "On" } else { "Off" };
            rows_html.push_str(&format!(
                r#"<tr><td class="col-room">{}</td><td><span class="badge {}">{}</span></td><td class="col-prov">{}</td></tr>"#,
                esc_html(&room),
                badge_class,
                badge_text,
                esc_html(&p),
            ));
        }
    }
    rows_html
}

pub(crate) fn journal_rows_html(dtos: &[JournalLineDto]) -> String {
    if dtos.is_empty() {
        return r#"<tr><td colspan="3" class="cell-empty"><em>Journal is empty</em></td></tr>"#
            .to_string();
    }
    let mut out = String::new();
    for line in dtos.iter().rev() {
        let kind = event_kind_snake(&line.kind);
        out.push_str(&format!(
            r#"<tr><td class="mono">{seq}</td><td class="mono">{ts}</td><td class="mono kind">{kind}</td></tr>"#,
            seq = line.sequence,
            ts = line.timestamp,
            kind = esc_html(&kind),
        ));
    }
    out
}

pub(crate) fn render_dashboard_page(
    security_banner: &str,
    journal_path_display: &str,
    lights_rows: &str,
    journal_rows: &str,
    last_log: &str,
) -> String {
    include_str!("dashboard.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%JOURNAL_PATH%%", journal_path_display)
        .replace("%%LIGHTS_ROWS%%", lights_rows)
        .replace("%%JOURNAL_ROWS%%", journal_rows)
        .replace("%%LAST_LOG%%", last_log)
        .replace("%%JOURNAL_LIMIT%%", &DASHBOARD_JOURNAL_ROWS.to_string())
}

fn kv_row_th(label: &str, value: &str) -> String {
    format!(
        r#"<tr><th>{}</th><td>{}</td></tr>"#,
        esc_html(label),
        esc_html(value),
    )
}

pub(crate) fn system_rusthome_rows(s: &system_info::SystemSnapshot) -> String {
    let mut rows = String::new();
    rows.push_str(&kv_row_th("Service", &s.service));
    rows.push_str(&kv_row_th(
        "rusthome-web version",
        &s.rusthome_version,
    ));
    rows.push_str(&kv_row_th("Listen address", &s.listen));
    rows.push_str(&kv_row_th("Data directory", &s.data_dir));
    rows.push_str(&kv_row_th("Journal file", &s.journal_path));
    let journal_meta = match (s.journal_file_present, s.journal_file_bytes) {
        (true, Some(b)) => format!("present — {}", system_info::fmt_bytes(b)),
        (true, None) => "present".to_string(),
        (false, _) => "missing (no events yet)".to_string(),
    };
    rows.push_str(&kv_row_th("Journal on disk", &journal_meta));
    rows
}

pub(crate) fn system_host_rows(s: &system_info::SystemSnapshot) -> String {
    let mut rows = String::new();
    rows.push_str(&kv_row_th(
        "Hostname",
        &system_info::opt_str(&s.hostname),
    ));
    rows.push_str(&kv_row_th(
        "OS",
        &system_info::opt_str(&s.os_long.clone().or(s.os_name.clone())),
    ));
    rows.push_str(&kv_row_th(
        "Kernel",
        &system_info::opt_str(&s.kernel),
    ));
    rows.push_str(&kv_row_th("CPU architecture", &s.cpu_arch));
    rows.push_str(&kv_row_th(
        "Uptime",
        &system_info::fmt_duration(s.uptime_secs),
    ));
    rows.push_str(&kv_row_th(
        "Load average",
        &format!(
            "{:.2} · {:.2} · {:.2} (1 / 5 / 15 min)",
            s.load_avg_1, s.load_avg_5, s.load_avg_15
        ),
    ));
    rows
}

pub(crate) fn system_resource_rows(s: &system_info::SystemSnapshot) -> String {
    let mut rows = String::new();
    let mem_pct = if s.memory_total_bytes > 0 {
        (s.memory_used_bytes as f64 / s.memory_total_bytes as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    rows.push_str(&format!(
        r#"<tr><th>Memory</th><td><div class="meter-wrap" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{p:.0}"><div class="meter-fill" style="width:{p:.1}%"></div></div><span class="meter-label">{used} / {total} ({p:.0}%)</span></td></tr>"#,
        p = mem_pct,
        used = esc_html(&system_info::fmt_bytes(s.memory_used_bytes)),
        total = esc_html(&system_info::fmt_bytes(s.memory_total_bytes)),
    ));

    let swap_s = if s.swap_total_bytes > 0 {
        format!(
            "{} / {}",
            system_info::fmt_bytes(s.swap_used_bytes),
            system_info::fmt_bytes(s.swap_total_bytes)
        )
    } else {
        "—".to_string()
    };
    rows.push_str(&kv_row_th("Swap", &swap_s));

    rows.push_str(&kv_row_th(
        "CPUs (logical)",
        &s.cpu_count.to_string(),
    ));
    rows.push_str(&kv_row_th(
        "CPU usage (global)",
        &format!("{:.1}%", s.cpu_usage_percent),
    ));

    let temp_s = s
        .cpu_temp_c_max
        .map(|t| format!("{t:.1} °C"))
        .unwrap_or_else(|| "—".to_string());
    rows.push_str(&kv_row_th("Temperature (sensors max)", &temp_s));

    let disk_s = match (
        s.disk_mount.as_deref(),
        s.disk_total_bytes,
        s.disk_available_bytes,
    ) {
        (Some(mount), Some(tot), Some(avail)) => format!(
            "{} — {} free of {} (data dir mount)",
            mount,
            system_info::fmt_bytes(avail),
            system_info::fmt_bytes(tot)
        ),
        _ => "— (could not map mount)".to_string(),
    };
    rows.push_str(&kv_row_th("Disk (data volume)", &disk_s));
    rows
}

fn tri_bool_label(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "yes",
        Some(false) => "no",
        None => "—",
    }
}

pub(crate) fn bluetooth_rows_html(s: &bluetooth_info::BluetoothSnapshot) -> String {
    let mut rows = String::new();
    if !s.notes.is_empty() {
        rows.push_str(&format!(
            r#"<tr><td colspan="2" class="cell-muted">{}</td></tr>"#,
            esc_html(&s.notes.join(" · ")),
        ));
    }
    if s.adapters.is_empty() && s.devices.is_empty() {
        return rows;
    }
    for a in &s.adapters {
        rows.push_str(&format!(
            r#"<tr><th colspan="2" class="subhdr">{}</th></tr>"#,
            esc_html(&a.hci_device),
        ));
        let addr = if a.address.is_empty() {
            "—"
        } else {
            a.address.as_str()
        };
        rows.push_str(&kv_row_th("Address", addr));
        let nm = if a.name.is_empty() {
            "—"
        } else {
            a.name.as_str()
        };
        rows.push_str(&kv_row_th("Name", nm));
        if let Some(ref c) = a.device_class {
            rows.push_str(&kv_row_th("Device class", c));
        }
        if let Some(b) = a.rfkill_soft_blocked {
            rows.push_str(&kv_row_th(
                "RF-kill (soft)",
                if b { "blocked" } else { "unblocked" },
            ));
        }
        rows.push_str(&kv_row_th("Powered", tri_bool_label(a.powered)));
        rows.push_str(&kv_row_th(
            "Discoverable",
            tri_bool_label(a.discoverable),
        ));
        rows.push_str(&kv_row_th("Pairable", tri_bool_label(a.pairable)));
    }
    if !s.devices.is_empty() {
        rows.push_str(r#"<tr><th colspan="2" class="subhdr">Known devices</th></tr>"#);
        for d in &s.devices {
            let mut td = if d.name.is_empty() {
                "—".to_string()
            } else {
                d.name.clone()
            };
            if let Some(p) = d.paired {
                td.push_str(if p {
                    " · paired: yes"
                } else {
                    " · paired: no"
                });
            }
            if let Some(c) = d.connected {
                td.push_str(if c {
                    " · connected: yes"
                } else {
                    " · connected: no"
                });
            }
            rows.push_str(&format!(
                r#"<tr><th class="mono">{}</th><td>{}</td></tr>"#,
                esc_html(&d.address),
                esc_html(&td),
            ));
        }
    }
    rows
}

pub(crate) fn render_system_page(
    security_banner: &str,
    rusthome: &str,
    host: &str,
    resources: &str,
    bluetooth: &str,
) -> String {
    include_str!("system.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%RUSTHOME_ROWS%%", rusthome)
        .replace("%%HOST_ROWS%%", host)
        .replace("%%RESOURCE_ROWS%%", resources)
        .replace("%%BLUETOOTH_ROWS%%", bluetooth)
}
