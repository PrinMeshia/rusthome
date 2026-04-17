//! Server-side HTML fragments and full page templates (`include_str!` from `../templates/`).

use rusthome_core::State as CoreState;

use crate::bluetooth_info;
use crate::journal::JournalLineDto;
use crate::system_info;
use crate::util::esc_html;

pub(crate) fn lights_rows_html(state: &CoreState, broker_available: bool) -> String {
    let mut rows_html = String::new();
    let rows = state.light_room_rows();
    let colspan = if broker_available { 4 } else { 3 };
    if rows.is_empty() {
        rows_html.push_str(&format!(
            r#"<tr><td colspan="{colspan}" class="cell-empty"><em>No rooms in projection yet</em></td></tr>"#,
        ));
    } else {
        for (room, on, prov) in rows {
            let p = prov
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "—".to_string());
            let badge_class = if on { "on" } else { "off" };
            let badge_text = if on { "On" } else { "Off" };
            let action_col = if broker_available {
                let btn_text = if on { "Turn Off" } else { "Turn On" };
                let action = if on { "turn_off" } else { "turn_on" };
                format!(
                    r#"<td><button class="btn-toggle" onclick="toggleLight('{room}',{on})"data-action="{action}">{btn_text}</button></td>"#,
                    room = esc_html(&room),
                    on = if on { "true" } else { "false" },
                    action = action,
                    btn_text = btn_text,
                )
            } else {
                String::new()
            };
            rows_html.push_str(&format!(
                r#"<tr><td class="col-room">{room}</td><td><span class="badge {cls}">{badge}</span></td><td class="col-prov">{prov}</td>{action}</tr>"#,
                room = esc_html(&room),
                cls = badge_class,
                badge = badge_text,
                prov = esc_html(&p),
                action = action_col,
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
        out.push_str(&format!(
            r#"<tr><td class="mono">{seq}</td><td class="mono">{ts}</td><td><span class="badge badge-{family}">{detail}</span></td></tr>"#,
            seq = line.sequence,
            ts = line.timestamp,
            family = line.family,
            detail = esc_html(&line.detail),
        ));
    }
    out
}

/// Tail length for the dashboard journal panel (SSR + JS refresh).
pub(crate) const DASHBOARD_JOURNAL_ROWS: usize = 40;

pub(crate) fn summary_cards_html(state: &CoreState, journal_count: usize) -> String {
    let rooms = state.light_room_rows().len();
    let lights_on = state.light_room_rows().iter().filter(|(_, on, _)| *on).count();
    let temp_count = state.temperature_readings().len();
    let contact_count = state.contact_states().len();
    let sensor_count = temp_count + contact_count;

    format!(
        r#"<div class="summary-card"><span class="summary-icon">{icon_rooms}</span><span class="summary-value">{rooms}</span><span class="summary-label">Rooms</span></div><div class="summary-card"><span class="summary-icon">{icon_light}</span><span class="summary-value">{lights_on}/{rooms}</span><span class="summary-label">Lights On</span></div><div class="summary-card"><span class="summary-icon">{icon_sensor}</span><span class="summary-value">{sensor_count}</span><span class="summary-label">Sensors</span></div><div class="summary-card"><span class="summary-icon">{icon_journal}</span><span class="summary-value">{journal_count}</span><span class="summary-label">Events</span></div>"#,
        icon_rooms = "\u{1F3E0}",
        icon_light = "\u{1F4A1}",
        icon_sensor = "\u{1F321}\u{FE0F}",
        icon_journal = "\u{1F4CB}",
    )
}

pub(crate) fn sensors_rows_html(state: &CoreState) -> String {
    let mut out = String::new();

    if state.temperature_readings().is_empty() && state.contact_states().is_empty() {
        return r#"<tr><td colspan="3" class="cell-empty"><em>No sensor data yet</em></td></tr>"#
            .to_string();
    }

    for (sensor_id, millideg) in state.temperature_readings() {
        let celsius = *millideg as f64 / 1000.0;
        out.push_str(&format!(
            r#"<tr><td class="col-room">{icon} {id}</td><td><span class="badge badge-fact">{val:.1}{deg}C</span></td><td class="col-prov">temperature</td></tr>"#,
            icon = "\u{1F321}\u{FE0F}",
            id = esc_html(sensor_id),
            val = celsius,
            deg = "\u{00B0}",
        ));
    }

    for (sensor_id, open) in state.contact_states() {
        let (state_text, badge_class) = if *open {
            ("Open", "badge-obs")
        } else {
            ("Closed", "badge-fact")
        };
        out.push_str(&format!(
            r#"<tr><td class="col-room">{icon} {id}</td><td><span class="badge {cls}">{st}</span></td><td class="col-prov">contact</td></tr>"#,
            icon = "\u{1F6AA}",
            id = esc_html(sensor_id),
            cls = badge_class,
            st = state_text,
        ));
    }

    out
}

pub(crate) fn render_dashboard_page(
    security_banner: &str,
    journal_path_display: &str,
    lights_rows: &str,
    journal_rows: &str,
    summary_cards: &str,
    sensors_rows: &str,
    broker_available: bool,
) -> String {
    include_str!("../templates/dashboard.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%JOURNAL_PATH%%", journal_path_display)
        .replace("%%LIGHTS_ROWS%%", lights_rows)
        .replace("%%JOURNAL_ROWS%%", journal_rows)
        .replace("%%SUMMARY_CARDS%%", summary_cards)
        .replace("%%SENSORS_ROWS%%", sensors_rows)
        .replace("%%JOURNAL_LIMIT%%", &DASHBOARD_JOURNAL_ROWS.to_string())
        .replace(
            "%%BROKER_AVAILABLE%%",
            if broker_available { "true" } else { "false" },
        )
}

pub(crate) fn render_sensors_page(
    security_banner: &str,
    temp_rows: &str,
    contact_rows: &str,
) -> String {
    include_str!("../templates/sensors.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%TEMPERATURE_ROWS%%", temp_rows)
        .replace("%%CONTACT_ROWS%%", contact_rows)
}

pub(crate) fn temperature_rows_html(state: &CoreState) -> String {
    if state.temperature_readings().is_empty() {
        return r#"<tr><td colspan="2" class="cell-empty"><em>No temperature sensors yet</em></td></tr>"#.to_string();
    }
    let mut out = String::new();
    for (sensor_id, millideg) in state.temperature_readings() {
        let celsius = *millideg as f64 / 1000.0;
        out.push_str(&format!(
            r#"<tr><td class="col-room">{icon} {id}</td><td><span class="badge badge-fact">{val:.1}{deg}C</span></td></tr>"#,
            icon = "\u{1F321}\u{FE0F}",
            id = esc_html(sensor_id),
            val = celsius,
            deg = "\u{00B0}",
        ));
    }
    out
}

pub(crate) fn contact_rows_html(state: &CoreState) -> String {
    if state.contact_states().is_empty() {
        return r#"<tr><td colspan="2" class="cell-empty"><em>No contact sensors yet</em></td></tr>"#.to_string();
    }
    let mut out = String::new();
    for (sensor_id, open) in state.contact_states() {
        let (state_text, badge_class) = if *open {
            ("Open", "badge-obs")
        } else {
            ("Closed", "badge-fact")
        };
        out.push_str(&format!(
            r#"<tr><td class="col-room">{icon} {id}</td><td><span class="badge {cls}">{st}</span></td></tr>"#,
            icon = "\u{1F6AA}",
            id = esc_html(sensor_id),
            cls = badge_class,
            st = state_text,
        ));
    }
    out
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
    include_str!("../templates/system.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%RUSTHOME_ROWS%%", rusthome)
        .replace("%%HOST_ROWS%%", host)
        .replace("%%RESOURCE_ROWS%%", resources)
        .replace("%%BLUETOOTH_ROWS%%", bluetooth)
}
