//! Server-side HTML fragments and full page templates (`include_str!` from `../templates/`).

use rusthome_core::State as CoreState;

use crate::bluetooth_info;
use crate::journal::JournalLineDto;
use crate::system_info;
use crate::util::{esc_attr, esc_html};
use rusthome_app::rusthome_file::Zigbee2MqttConfig;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavPage {
    Dashboard,
    Sensors,
    System,
}

pub(crate) fn main_nav_html(active: NavPage) -> String {
    let mut out =
        String::from(r#"<nav class="app-nav" aria-label="Navigation principale">"#);
    let items = [
        (NavPage::Dashboard, "/", "Tableau de bord"),
        (NavPage::Sensors, "/sensors", "Capteurs"),
        (NavPage::System, "/system", "Système"),
    ];
    for (page, href, label) in items {
        if page == active {
            out.push_str(&format!(
                r#"<span class="app-nav-item is-active" aria-current="page">{}</span>"#,
                esc_html(label)
            ));
        } else {
            out.push_str(&format!(
                r#"<a class="app-nav-item" href="{}">{}</a>"#,
                esc_html(href),
                esc_html(label)
            ));
        }
    }
    out.push_str("</nav>");
    out
}

fn dev_footer_inner(links: &[(&str, &str)]) -> String {
    let mut body = String::new();
    for (href, label) in links {
        body.push_str(&format!(
            r#"<a href="{}">{}</a>"#,
            esc_html(href),
            esc_html(label)
        ));
    }
    format!(
        r#"<footer class="app-footer-dev"><strong>API</strong>{}</footer>"#,
        body
    )
}

pub(crate) fn dev_footer_dashboard() -> String {
    dev_footer_inner(&[
        ("/api/state", "État JSON"),
        ("/api/journal?limit=40", "Journal JSON"),
        ("/api/health", "Santé"),
    ])
}

pub(crate) fn dev_footer_sensors() -> String {
    dev_footer_inner(&[
        ("/api/state", "État JSON"),
        ("/api/sensor-display", "Métadonnées capteurs JSON"),
        ("/api/sensor-display/sync-from-state", "POST synchro libellés"),
        ("/api/observation", "POST observation MQTT test"),
        ("/docs/mqtt-contract", "Contrat MQTT"),
    ])
}

pub(crate) fn dev_footer_system() -> String {
    dev_footer_inner(&[
        ("/api/system", "Système JSON"),
        ("/api/bluetooth", "Bluetooth JSON"),
        ("/api/bluetooth/device?addr=…&scan=10", "Présence MAC + scan opt."),
        ("/api/bluetooth/info?addr=…", "Infos bluetoothctl"),
        ("/api/zigbee2mqtt/bridge", "GET état permit_join Z2M"),
        ("/api/zigbee2mqtt/permit_join", "POST permit join Z2M"),
        ("/api/health", "Santé"),
    ])
}

pub(crate) fn lights_rows_html(state: &CoreState, broker_available: bool) -> String {
    let mut rows_html = String::new();
    let rows = state.light_room_rows();
    let colspan = if broker_available { 4 } else { 3 };
    if rows.is_empty() {
        rows_html.push_str(&format!(
            r#"<tr><td colspan="{colspan}" class="cell-empty"><em>Aucune pièce dans la projection</em></td></tr>"#,
        ));
    } else {
        for (room, on, prov) in rows {
            let p = prov
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "—".to_string());
            let badge_class = if on { "on" } else { "off" };
            let badge_text = if on { "Allumée" } else { "Éteinte" };
            let action_col = if broker_available {
                let aria = format!(
                    "Lumière {}, {}",
                    room,
                    if on { "allumée" } else { "éteinte" }
                );
                format!(
                    r#"<td><button type="button" class="light-switch" role="switch" aria-checked="{checked}" data-room="{room}" data-on="{on_attr}" aria-label="{aria}"></button></td>"#,
                    checked = if on { "true" } else { "false" },
                    room = esc_attr(&room),
                    on_attr = if on { "true" } else { "false" },
                    aria = esc_attr(&aria),
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
        return r#"<tr><td colspan="3" class="cell-empty"><em>Le journal est vide</em></td></tr>"#
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
    let humidity_count = state.humidity_readings().len();
    let contact_count = state.contact_states().len();
    let sensor_count = temp_count + humidity_count + contact_count;

    format!(
        r#"<div class="summary-card"><span class="summary-icon">{icon_rooms}</span><span class="summary-value">{rooms}</span><span class="summary-label">Pièces</span></div><div class="summary-card"><span class="summary-icon">{icon_light}</span><span class="summary-value">{lights_on}/{rooms}</span><span class="summary-label">Lampes</span></div><div class="summary-card"><span class="summary-icon">{icon_sensor}</span><span class="summary-value">{sensor_count}</span><span class="summary-label">Capteurs</span></div><div class="summary-card"><span class="summary-icon">{icon_journal}</span><span class="summary-value">{journal_count}</span><span class="summary-label">Événements</span></div>"#,
        icon_rooms = "\u{1F3E0}",
        icon_light = "\u{1F4A1}",
        icon_sensor = "\u{1F321}\u{FE0F}",
        icon_journal = "\u{1F4CB}",
    )
}

pub(crate) fn sensors_rows_html(state: &CoreState) -> String {
    let mut out = String::new();

    if state.temperature_readings().is_empty()
        && state.humidity_readings().is_empty()
        && state.contact_states().is_empty()
    {
        return r#"<tr><td colspan="3" class="cell-empty"><em>Aucune donnée capteur</em></td></tr>"#
            .to_string();
    }

    for (sensor_id, millideg) in state.temperature_readings() {
        let celsius = *millideg as f64 / 1000.0;
        out.push_str(&format!(
            r#"<tr><td class="col-room">{icon} {id}</td><td><span class="badge badge-fact">{val:.1}{deg}C</span></td><td class="col-prov">température</td></tr>"#,
            icon = "\u{1F321}\u{FE0F}",
            id = esc_html(sensor_id),
            val = celsius,
            deg = "\u{00B0}",
        ));
    }

    for (sensor_id, permille) in state.humidity_readings() {
        let pct = *permille as f64 / 10.0;
        out.push_str(&format!(
            r#"<tr><td class="col-room">{icon} {id}</td><td><span class="badge badge-fact">{val:.1} %</span></td><td class="col-prov">humidité</td></tr>"#,
            icon = "\u{1F4A7}",
            id = esc_html(sensor_id),
            val = pct,
        ));
    }

    for (sensor_id, open) in state.contact_states() {
        let (state_text, badge_class) = if *open {
            ("Ouvert", "badge-obs")
        } else {
            ("Fermé", "badge-fact")
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

pub(crate) fn broker_pill_html(broker_available: bool) -> &'static str {
    if broker_available {
        r#"<span id="broker-pill" class="broker-pill broker-ok" title="Broker MQTT intégré : commandes lumière et publications capteurs">MQTT prêt</span>"#
    } else {
        r#"<span id="broker-pill" class="broker-pill broker-off" title="Pas de broker : lancez rusthome serve pour publier des commandes et des observations">Lecture seule</span>"#
    }
}

pub(crate) fn render_dashboard_page(
    security_banner: &str,
    journal_path_display: &str,
    lights_rows: &str,
    journal_rows: &str,
    summary_cards: &str,
    sensors_rows: &str,
    broker_available: bool,
    live_push: bool,
) -> String {
    let dashboard_cfg = format!(
        r#"{{"journalLimit":{},"brokerAvailable":{},"livePush":{}}}"#,
        DASHBOARD_JOURNAL_ROWS,
        if broker_available { "true" } else { "false" },
        if live_push { "true" } else { "false" }
    );
    include_str!("../templates/dashboard.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%JOURNAL_PATH%%", journal_path_display)
        .replace("%%LIGHTS_ROWS%%", lights_rows)
        .replace("%%JOURNAL_ROWS%%", journal_rows)
        .replace("%%SUMMARY_CARDS%%", summary_cards)
        .replace("%%SENSORS_ROWS%%", sensors_rows)
        .replace("%%RH_DASHBOARD_CONFIG%%", &dashboard_cfg)
        .replace("%%BROKER_PILL%%", broker_pill_html(broker_available))
        .replace("%%MAIN_NAV%%", &main_nav_html(NavPage::Dashboard))
        .replace("%%DEV_FOOTER%%", &dev_footer_dashboard())
}

pub(crate) fn render_sensors_page(
    security_banner: &str,
    temp_rows: &str,
    humidity_rows: &str,
    contact_rows: &str,
    broker_available: bool,
    live_push: bool,
) -> String {
    let sensors_cfg = format!(
        r#"{{"brokerAvailable":{},"livePush":{}}}"#,
        if broker_available { "true" } else { "false" },
        if live_push { "true" } else { "false" },
    );
    include_str!("../templates/sensors.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%TEMPERATURE_ROWS%%", temp_rows)
        .replace("%%HUMIDITY_ROWS%%", humidity_rows)
        .replace("%%CONTACT_ROWS%%", contact_rows)
        .replace("%%BROKER_PILL%%", broker_pill_html(broker_available))
        .replace("%%RH_SENSORS_CONFIG%%", &sensors_cfg)
        .replace("%%MAIN_NAV%%", &main_nav_html(NavPage::Sensors))
        .replace("%%DEV_FOOTER%%", &dev_footer_sensors())
}

pub(crate) fn temperature_rows_html(state: &CoreState) -> String {
    if state.temperature_readings().is_empty() {
        return r#"<tr><td colspan="4" class="cell-empty"><em>Aucun capteur de température</em></td></tr>"#
            .to_string();
    }
    let mut out = String::new();
    for (sensor_id, millideg) in state.temperature_readings() {
        let celsius = *millideg as f64 / 1000.0;
        out.push_str(&format!(
            r#"<tr><td class="sensor-cell-meta"></td><td class="sensor-cell-meta"></td><td class="sensor-id-cell mono">{id}</td><td><span class="badge badge-fact">{val:.1}{deg}C</span></td></tr>"#,
            id = esc_html(sensor_id),
            val = celsius,
            deg = "\u{00B0}",
        ));
    }
    out
}

pub(crate) fn humidity_rows_html(state: &CoreState) -> String {
    if state.humidity_readings().is_empty() {
        return r#"<tr><td colspan="4" class="cell-empty"><em>Aucun capteur d&apos;humidité</em></td></tr>"#
            .to_string();
    }
    let mut out = String::new();
    for (sensor_id, permille) in state.humidity_readings() {
        let pct = *permille as f64 / 10.0;
        out.push_str(&format!(
            r#"<tr><td class="sensor-cell-meta"></td><td class="sensor-cell-meta"></td><td class="sensor-id-cell mono">{id}</td><td><span class="badge badge-fact">{val:.1} %</span></td></tr>"#,
            id = esc_html(sensor_id),
            val = pct,
        ));
    }
    out
}

pub(crate) fn contact_rows_html(state: &CoreState) -> String {
    if state.contact_states().is_empty() {
        return r#"<tr><td colspan="4" class="cell-empty"><em>Aucun contact</em></td></tr>"#.to_string();
    }
    let mut out = String::new();
    for (sensor_id, open) in state.contact_states() {
        let (state_text, badge_class) = if *open {
            ("Ouvert", "badge-obs")
        } else {
            ("Fermé", "badge-fact")
        };
        out.push_str(&format!(
            r#"<tr><td class="sensor-cell-meta"></td><td class="sensor-cell-meta"></td><td class="sensor-id-cell mono">{id}</td><td><span class="badge {cls}">{st}</span></td></tr>"#,
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
        "Version rusthome-web",
        &s.rusthome_version,
    ));
    rows.push_str(&kv_row_th("Adresse d’écoute", &s.listen));
    rows.push_str(&kv_row_th("Répertoire de données", &s.data_dir));
    rows.push_str(&kv_row_th("Fichier journal", &s.journal_path));
    let journal_meta = match (s.journal_file_present, s.journal_file_bytes) {
        (true, Some(b)) => format!("présent — {}", system_info::fmt_bytes(b)),
        (true, None) => "présent".to_string(),
        (false, _) => "absent (pas encore d’événements)".to_string(),
    };
    rows.push_str(&kv_row_th("Journal sur disque", &journal_meta));
    rows
}

pub(crate) fn system_host_rows(s: &system_info::SystemSnapshot) -> String {
    let mut rows = String::new();
    rows.push_str(&kv_row_th(
        "Nom d’hôte",
        &system_info::opt_str(&s.hostname),
    ));
    rows.push_str(&kv_row_th(
        "OS",
        &system_info::opt_str(&s.os_long.clone().or(s.os_name.clone())),
    ));
    rows.push_str(&kv_row_th(
        "Noyau",
        &system_info::opt_str(&s.kernel),
    ));
    rows.push_str(&kv_row_th("Architecture CPU", &s.cpu_arch));
    rows.push_str(&kv_row_th(
        "Durée de fonctionnement",
        &system_info::fmt_duration(s.uptime_secs),
    ));
    rows.push_str(&kv_row_th(
        "Charge moyenne",
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
        r#"<tr><th>Mémoire</th><td><div class="meter-wrap" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{p:.0}"><div class="meter-fill" style="width:{p:.1}%"></div></div><span class="meter-label">{used} / {total} ({p:.0}%)</span></td></tr>"#,
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
        "CPU (logiques)",
        &s.cpu_count.to_string(),
    ));
    rows.push_str(&kv_row_th(
        "Utilisation CPU",
        &format!("{:.1} %", s.cpu_usage_percent),
    ));

    let temp_s = s
        .cpu_temp_c_max
        .map(|t| format!("{t:.1} °C"))
        .unwrap_or_else(|| "—".to_string());
    rows.push_str(&kv_row_th("Température (max capteurs)", &temp_s));

    let disk_s = match (
        s.disk_mount.as_deref(),
        s.disk_total_bytes,
        s.disk_available_bytes,
    ) {
        (Some(mount), Some(tot), Some(avail)) => format!(
            "{} — {} libres sur {} (volume données)",
            mount,
            system_info::fmt_bytes(avail),
            system_info::fmt_bytes(tot)
        ),
        _ => "— (montage introuvable)".to_string(),
    };
    rows.push_str(&kv_row_th("Disque (données)", &disk_s));
    rows
}

fn tri_bool_label(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "oui",
        Some(false) => "non",
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
        rows.push_str(&kv_row_th("Adresse", addr));
        let nm = if a.name.is_empty() {
            "—"
        } else {
            a.name.as_str()
        };
        rows.push_str(&kv_row_th("Nom", nm));
        if let Some(ref c) = a.device_class {
            rows.push_str(&kv_row_th("Classe d’appareil", c));
        }
        if let Some(b) = a.rfkill_soft_blocked {
            rows.push_str(&kv_row_th(
                "RF-kill (logiciel)",
                if b { "bloqué" } else { "débloqué" },
            ));
        }
        rows.push_str(&kv_row_th("Alimenté", tri_bool_label(a.powered)));
        rows.push_str(&kv_row_th(
            "Visible",
            tri_bool_label(a.discoverable),
        ));
        rows.push_str(&kv_row_th("Appairable", tri_bool_label(a.pairable)));
    }
    if !s.devices.is_empty() {
        rows.push_str(r#"<tr><th colspan="2" class="subhdr">Appareils connus</th></tr>"#);
        for d in &s.devices {
            let mut td = if d.name.is_empty() {
                "—".to_string()
            } else {
                d.name.clone()
            };
            if let Some(p) = d.paired {
                td.push_str(if p {
                    " · appairé : oui"
                } else {
                    " · appairé : non"
                });
            }
            if let Some(c) = d.connected {
                td.push_str(if c {
                    " · connecté : oui"
                } else {
                    " · connecté : non"
                });
            }
            rows.push_str(&format!(
                r#"<tr><th class="mono">{addr}</th><td><span class="bt-device-summary">{td}</span> <button type="button" class="bt-device-info" data-address="{addr_attr}" aria-label="Détails Bluetooth {addr_attr}">Détails</button></td></tr>"#,
                addr = esc_html(&d.address),
                td = esc_html(&td),
                addr_attr = esc_html(&d.address),
            ));
        }
    }
    rows
}

pub(crate) fn system_serial_rows_html(ports: &[system_info::SerialPortInfo]) -> String {
    if ports.is_empty() {
        return r#"<tr><td colspan="2" class="cell-empty"><em>Aucun port <code class="mono">ttyACM*</code> / <code class="mono">ttyUSB*</code> détecté. Branchez le dongle ou vérifiez les pilotes.</em></td></tr>"#
            .to_string();
    }
    let mut rows = String::new();
    for p in ports {
        let vid_pid = match (&p.vendor_id, &p.product_id) {
            (Some(v), Some(i)) => format!("0x{v} / 0x{i}"),
            (Some(v), None) => format!("0x{v} / —"),
            (None, Some(i)) => format!("— / 0x{i}"),
            (None, None) => "—".to_string(),
        };
        let by_id = p
            .by_id_name
            .as_deref()
            .map(esc_html)
            .unwrap_or_else(|| "—".to_string());
        let prod = p
            .product_label
            .as_deref()
            .map(esc_html)
            .unwrap_or_else(|| "—".to_string());
        let mut extra = String::new();
        if p.maybe_conbee_hint {
            extra.push_str(r#" <span class="badge badge-fact" title="Vendor id 0x1CF1 — Dresden Elektronik (famille Conbee)">indice Conbee</span>"#);
        }
        if !p.notes.is_empty() {
            extra.push_str(&format!(
                r#" <span class="cell-muted">{}</span>"#,
                esc_html(&p.notes.join(" · "))
            ));
        }
        rows.push_str(&format!(
            r#"<tr><th class="mono">{}</th><td>by-id: {} · VID/PID: {} · {} {}</td></tr>"#,
            esc_html(&p.device),
            by_id,
            esc_html(&vid_pid),
            prod,
            extra,
        ));
    }
    rows
}

pub(crate) fn zigbee2mqtt_panel_html(cfg: &Zigbee2MqttConfig, broker_available: bool) -> String {
    let prefix = cfg.resolved_topic_prefix();
    let prefix_esc = esc_html(&prefix);
    let topic_esc = esc_html(&format!("{prefix}/bridge/request"));
    let info_topic_esc = esc_html(&format!("{}/bridge/info", prefix.trim_matches('/')));
    let secs = cfg.resolved_permit_join_seconds();
    let broker_note = if broker_available {
        r#"<p class="bt-hint">Broker MQTT intégré : la requête est publiée sur le topic bridge Zigbee2MQTT.</p>"#
    } else {
        r#"<p class="bt-hint"><span class="badge badge-error">Broker indisponible</span> — lancez <code class="mono">rusthome serve</code> sans <code class="mono">--no-broker</code> pour publier vers Zigbee2MQTT.</p>"#
    };
    let disabled = if broker_available { "" } else { " disabled" };
    format!(
        r##"<section class="card wide z2m-card" id="z2m-section">
  <h2>Zigbee2MQTT — appairage</h2>
  {broker_note}
  <p class="bt-hint">Préfixe MQTT : <code class="mono">{prefix_esc}</code> → requêtes <code class="mono">{topic_esc}</code>. Zigbee2MQTT doit être connecté au <strong>même broker</strong> que rusthome. Documentation : <span class="mono">docs/zigbee-conbee.md</span>.</p>
  <p class="bt-hint z2m-hint-usb">La section <strong>Ports série USB</strong> (ci-dessus) reflète le matériel branché. La pastille d&apos;appairage utilise uniquement le flux MQTT <code class="mono">{info_topic_esc}</code> (émis par Zigbee2MQTT) : voir le même dongle en USB n&apos;implique pas encore que Z2M publie sur ce broker.</p>
  <div class="z2m-join-line" id="z2m-join-line" role="status" aria-live="polite">
    <span class="z2m-join-label">&Eacute;tat r&eacute;seau Zigbee (appairage)</span>
    <span id="z2m-permit-join-badge" class="z2m-permit-join-badge" data-state="unknown">Chargement…</span>
  </div>
  <div class="z2m-row">
    <button type="button" id="z2m-permit-btn" class="bt-mac-btn"{disabled}>Autoriser l&apos;appairage ({secs} s)</button>
    <span id="z2m-permit-status" class="cell-muted" role="status" aria-live="polite"></span>
  </div>
</section>"##,
        broker_note = broker_note,
        prefix_esc = prefix_esc,
        topic_esc = topic_esc,
        info_topic_esc = info_topic_esc,
        secs = secs,
        disabled = disabled,
    )
}

pub(crate) fn render_system_page(
    security_banner: &str,
    rusthome: &str,
    host: &str,
    resources: &str,
    serial: &str,
    zigbee2mqtt_panel: &str,
    bluetooth: &str,
) -> String {
    include_str!("../templates/system.html")
        .replace("%%SECURITY_BANNER%%", security_banner)
        .replace("%%RUSTHOME_ROWS%%", rusthome)
        .replace("%%HOST_ROWS%%", host)
        .replace("%%RESOURCE_ROWS%%", resources)
        .replace("%%SERIAL_ROWS%%", serial)
        .replace("%%ZIGBEE2MQTT_PANEL%%", zigbee2mqtt_panel)
        .replace("%%BLUETOOTH_ROWS%%", bluetooth)
        .replace("%%MAIN_NAV%%", &main_nav_html(NavPage::System))
        .replace("%%DEV_FOOTER%%", &dev_footer_system())
}
