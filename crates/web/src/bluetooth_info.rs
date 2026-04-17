//! Bluetooth inventory for the system dashboard (Linux: sysfs + optional `bluetoothctl`).
//!
//! Optional **short discovery** (`bluetoothctl scan on`) can run before a MAC lookup so devices
//! **in range** may appear without prior pairing (similar in spirit to Jeedom “BLE scan” presence).
//! Pairing and GATT are not implemented here.

use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct BluetoothSnapshot {
    pub adapters: Vec<BluetoothAdapter>,
    /// Known devices from `bluetoothctl devices` (+ Paired / Connected when available).
    pub devices: Vec<BluetoothKnownDevice>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BluetoothAdapter {
    pub hci_device: String,
    pub address: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    /// Soft block from rfkill (first Bluetooth-type entry), if detectable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rfkill_soft_blocked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub powered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discoverable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairable: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BluetoothKnownDevice {
    pub address: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
}

/// Result of looking up a MAC against `bluetoothctl devices` (optionally after a discovery scan).
#[derive(Debug, Clone, Serialize)]
pub struct BluetoothDeviceLookup {
    pub mac_input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_normalized: Option<String>,
    pub valid_format: bool,
    /// `true` if this MAC appears in BlueZ device list (`bluetoothctl devices`) at lookup time.
    pub in_known_devices: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
    /// A `bluetoothctl scan on` phase ran before listing (Jeedom-style: see unpaired devices in range).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub scan_performed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_seconds: Option<u32>,
    /// `false` on non-Linux builds.
    pub platform_supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Fields read from `bluetoothctl info <MAC>` (when BlueZ knows the device).
#[derive(Debug, Clone, Serialize, Default)]
pub struct BluetoothDeviceInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_normalized: Option<String>,
    pub platform_supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bonded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_pairing: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub uuids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rssi: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_percentage: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalias: Option<String>,
}

#[derive(Default)]
struct BluetoothCtlShow {
    powered: Option<bool>,
    discoverable: Option<bool>,
    pairable: Option<bool>,
}

/// Rich device attributes from `bluetoothctl info <MAC>` (best effort).
pub fn device_info(mac_input: &str) -> BluetoothDeviceInfo {
    #[cfg(not(target_os = "linux"))]
    {
        return BluetoothDeviceInfo {
            mac_normalized: normalize_mac(mac_input),
            platform_supported: false,
            error: Some("La commande info Bluetooth n’est disponible que sous Linux.".into()),
            ..Default::default()
        };
    }
    #[cfg(target_os = "linux")]
    {
        device_info_linux(mac_input)
    }
}

#[cfg(target_os = "linux")]
fn device_info_linux(mac_input: &str) -> BluetoothDeviceInfo {
    let Some(norm) = normalize_mac(mac_input) else {
        return BluetoothDeviceInfo {
            mac_normalized: None,
            platform_supported: true,
            error: Some("MAC invalide.".into()),
            ..Default::default()
        };
    };
    if !bluetoothctl_available() {
        return BluetoothDeviceInfo {
            mac_normalized: Some(norm),
            platform_supported: true,
            error: Some("bluetoothctl absent.".into()),
            ..Default::default()
        };
    }
    match run_bt_cmd(&["info", &norm]) {
        Ok(text) => parse_bluetoothctl_info(&norm, &text),
        Err(e) => BluetoothDeviceInfo {
            mac_normalized: Some(norm),
            platform_supported: true,
            error: Some(e),
            ..Default::default()
        },
    }
}

fn parse_bluetoothctl_info(mac: &str, text: &str) -> BluetoothDeviceInfo {
    let mut i = BluetoothDeviceInfo {
        mac_normalized: Some(mac.to_string()),
        platform_supported: true,
        ..Default::default()
    };
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("Device ") {
            continue;
        }
        let Some(pos) = line.find(':') else {
            continue;
        };
        let key = line[..pos].trim();
        let val = line[pos + 1..].trim();
        match key {
            "Name" => i.name = Some(val.to_string()),
            "Alias" => i.alias = Some(val.to_string()),
            "Class" => i.device_class = Some(val.to_string()),
            "Icon" => i.icon = Some(val.to_string()),
            "Paired" => i.paired = Some(val.eq_ignore_ascii_case("yes")),
            "Bonded" => i.bonded = Some(val.eq_ignore_ascii_case("yes")),
            "Trusted" => i.trusted = Some(val.eq_ignore_ascii_case("yes")),
            "Blocked" => i.blocked = Some(val.eq_ignore_ascii_case("yes")),
            "Connected" => i.connected = Some(val.eq_ignore_ascii_case("yes")),
            "LegacyPairing" => i.legacy_pairing = Some(val.eq_ignore_ascii_case("yes")),
            "UUID" => i.uuids.push(val.to_string()),
            "RSSI" => {
                let v = val.trim();
                if let Ok(n) = v.parse::<i32>() {
                    i.rssi = Some(n);
                } else if let Some(open) = v.rfind('(') {
                    let inner = v[open + 1..].trim_end_matches(')').trim();
                    if let Ok(n) = inner.parse::<i32>() {
                        i.rssi = Some(n);
                    }
                }
            }
            "Battery Percentage" => {
                if let Some(pct) = val.split_whitespace().find_map(|s| {
                    s.trim_end_matches(')').parse::<u8>().ok()
                }) {
                    i.battery_percentage = Some(pct.min(100));
                } else if let Some(hex) = val.strip_prefix("0x").and_then(|s| s.split_whitespace().next()) {
                    if let Ok(n) = u8::from_str_radix(hex, 16) {
                        i.battery_percentage = Some(n.min(100));
                    }
                }
            }
            "ManufacturerData" | "ManufacturerData Key" => {
                i.manufacturer_data = Some(val.to_string());
            }
            "Modalias" => i.modalias = Some(val.to_string()),
            _ => {}
        }
    }
    i
}

pub fn snapshot() -> BluetoothSnapshot {
    #[cfg(target_os = "linux")]
    {
        snapshot_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        BluetoothSnapshot {
            adapters: vec![],
            devices: vec![],
            notes: vec!["Bluetooth inventory is only implemented on Linux.".to_string()],
        }
    }
}

/// Normalize user input to `AA:BB:CC:DD:EE:FF` (uppercase) or `None` if invalid.
pub fn normalize_mac(input: &str) -> Option<String> {
    let mut s = input.trim().replace('-', ":").replace(' ', "");
    if !s.contains(':') && s.len() == 12 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut out = String::with_capacity(17);
        for (i, c) in s.chars().enumerate() {
            if i > 0 && i % 2 == 0 {
                out.push(':');
            }
            out.push(c.to_ascii_uppercase());
        }
        s = out;
    } else {
        s = s.to_ascii_uppercase();
    }
    looks_like_bt_mac(&s).then_some(s)
}

/// `scan_before_sec`: if `Some(n)` with n ≥ 5, runs a short `bluetoothctl scan on` first (max 45 s)
/// so devices in range can show up **without** being paired first (Jeedom-like behaviour).
pub fn lookup_device(mac_input: &str, scan_before_sec: Option<u32>) -> BluetoothDeviceLookup {
    let mac_trim = mac_input.trim().to_string();
    let mac_normalized = normalize_mac(&mac_trim);
    let scan_before_sec = scan_before_sec.and_then(|s| (s >= 5).then_some(s.min(45)));

    #[cfg(not(target_os = "linux"))]
    {
        return BluetoothDeviceLookup {
            mac_input: mac_trim,
            mac_normalized,
            valid_format: mac_normalized.is_some(),
            in_known_devices: false,
            name: None,
            paired: None,
            connected: None,
            scan_performed: false,
            scan_seconds: None,
            platform_supported: false,
            note: Some(
                "La détection Bluetooth (BlueZ) n’est implémentée que sous Linux.".to_string(),
            ),
        };
    }

    #[cfg(target_os = "linux")]
    lookup_device_linux(mac_trim, mac_normalized, scan_before_sec)
}

#[cfg(target_os = "linux")]
fn lookup_device_linux(
    mac_trim: String,
    mac_normalized: Option<String>,
    scan_before_sec: Option<u32>,
) -> BluetoothDeviceLookup {
    let Some(needle) = mac_normalized else {
        return BluetoothDeviceLookup {
            mac_input: mac_trim,
            mac_normalized: None,
            valid_format: false,
            in_known_devices: false,
            name: None,
            paired: None,
            connected: None,
            scan_performed: false,
            scan_seconds: None,
            platform_supported: true,
            note: Some(
                "Format MAC invalide : utilisez AA:BB:CC:DD:EE:FF (ou 12 chiffres hex sans séparateurs)."
                    .to_string(),
            ),
        };
    };

    let mut scan_performed = false;
    let mut scan_seconds = None;
    let mut scan_note: Option<String> = None;

    if let Some(secs) = scan_before_sec {
        scan_performed = true;
        scan_seconds = Some(secs);
        let _ = run_bt_cmd(&["power", "on"]);
        if let Err(e) = run_bluetooth_discovery_scan(secs) {
            scan_note = Some(format!(
                "Découverte BLE : {e} — la liste peut rester inchangée (droits BlueZ, adaptateur coupé, etc.)."
            ));
        }
    }

    let u = needle.to_ascii_uppercase();
    let snap = snapshot_linux();
    if let Some(d) = snap
        .devices
        .iter()
        .find(|d| d.address.to_ascii_uppercase() == u)
    {
        BluetoothDeviceLookup {
            mac_input: mac_trim,
            mac_normalized: Some(needle),
            valid_format: true,
            in_known_devices: true,
            name: if d.name.is_empty() {
                None
            } else {
                Some(d.name.clone())
            },
            paired: d.paired,
            connected: d.connected,
            scan_performed,
            scan_seconds,
            platform_supported: true,
            note: scan_note,
        }
    } else {
        let base = if scan_performed {
            "Même après une courte découverte BLE, cette adresse n’apparaît pas dans la liste BlueZ. \
             Causes fréquentes : randomisation d’adresse (téléphone), appareil hors de portée ou éteint, \
             ou scan refusé (utilisateur `bluetooth` / droits)."
        } else {
            "Cette adresse n’apparaît pas dans la liste BlueZ actuelle. Choisissez une **découverte BLE** \
             ci-dessous (type Jeedom : repère des appareils à portée sans appairage), ou appairez l’appareil \
             dans les paramètres système."
        };
        let mut note = base.to_string();
        if let Some(s) = scan_note {
            note = format!("{s} {note}");
        }
        BluetoothDeviceLookup {
            mac_input: mac_trim,
            mac_normalized: Some(needle),
            valid_format: true,
            in_known_devices: false,
            name: None,
            paired: None,
            connected: None,
            scan_performed,
            scan_seconds,
            platform_supported: true,
            note: Some(note),
        }
    }
}

#[cfg(target_os = "linux")]
fn snapshot_linux() -> BluetoothSnapshot {
    let mut notes = Vec::new();
    let rfkill_bt = read_bluetooth_rfkill_soft_blocked();
    let mut adapters = list_sysfs_adapters(rfkill_bt);

    if bluetoothctl_available() {
        let mut any_ctl_ok = false;
        for a in &mut adapters {
            if a.address.is_empty() {
                continue;
            }
            match run_bt_cmd(&["show", a.address.trim()]) {
                Ok(text) => {
                    any_ctl_ok = true;
                    merge_ctl_show(a, &text);
                }
                Err(e) => {
                    notes.push(format!("bluetoothctl show {}: {}", a.address, e));
                }
            }
        }
        // Default adapter: `bluetoothctl show` without MAC (helps when `show <addr>` fails).
        if adapters.len() == 1 && !any_ctl_ok {
            if let Ok(text) = run_bt_cmd(&["show"]) {
                merge_ctl_show(&mut adapters[0], &text);
                any_ctl_ok = true;
            }
        }
        if !any_ctl_ok && !adapters.is_empty() && notes.is_empty() {
            notes.push("bluetoothctl did not return controller details (is `bluetooth` running?).".into());
        }
    } else {
        notes.push(
            "Install BlueZ and ensure `bluetoothctl` is in PATH for Powered / Discoverable / Pairable."
                .to_string(),
        );
    }

    if adapters.is_empty() {
        notes.push("No hci* under /sys/class/bluetooth (no adapter or driver not loaded).".into());
    }

    let devices = if bluetoothctl_available() {
        list_known_bt_devices(&mut notes)
    } else {
        vec![]
    };

    BluetoothSnapshot {
        adapters,
        devices,
        notes,
    }
}

#[cfg(target_os = "linux")]
fn merge_ctl_show(a: &mut BluetoothAdapter, text: &str) {
    let p = parse_bluetoothctl_show(text);
    a.powered = p.powered.or(a.powered);
    a.discoverable = p.discoverable.or(a.discoverable);
    a.pairable = p.pairable.or(a.pairable);
}

#[cfg(target_os = "linux")]
fn read_file_trim(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path.as_ref())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "linux")]
fn list_sysfs_adapters(rfkill_soft_blocked: Option<bool>) -> Vec<BluetoothAdapter> {
    let mut out = Vec::new();
    let base = Path::new("/sys/class/bluetooth");
    let Ok(rd) = std::fs::read_dir(base) else {
        return out;
    };
    for e in rd.flatten() {
        let hci = e.file_name().to_string_lossy().into_owned();
        if !hci.starts_with("hci") {
            continue;
        }
        let p = e.path();
        let address = read_file_trim(p.join("address")).unwrap_or_default();
        let name = read_file_trim(p.join("name")).unwrap_or_default();
        let device_class = read_file_trim(p.join("class"))
            .or_else(|| read_file_trim(p.join("device/class")));

        out.push(BluetoothAdapter {
            hci_device: hci,
            address,
            name,
            device_class,
            rfkill_soft_blocked,
            powered: None,
            discoverable: None,
            pairable: None,
        });
    }
    out.sort_by(|a, b| a.hci_device.cmp(&b.hci_device));
    out
}

#[cfg(target_os = "linux")]
fn read_bluetooth_rfkill_soft_blocked() -> Option<bool> {
    let rfkill_root = Path::new("/sys/class/rfkill");
    let Ok(rd) = std::fs::read_dir(rfkill_root) else {
        return None;
    };
    for e in rd.flatten() {
        let p = e.path();
        let kind = read_file_trim(p.join("type"))?;
        if !kind.eq_ignore_ascii_case("bluetooth") {
            continue;
        }
        let soft = read_file_trim(p.join("soft"))?;
        return Some(soft == "1");
    }
    None
}

#[cfg(target_os = "linux")]
fn bluetoothctl_available() -> bool {
    Command::new("bluetoothctl").arg("--version").output().is_ok()
}

#[cfg(target_os = "linux")]
fn has_timeout_bin() -> bool {
    Path::new("/usr/bin/timeout").exists() || Path::new("/bin/timeout").exists()
}

#[cfg(target_os = "linux")]
fn run_bt_cmd(args: &[&str]) -> Result<String, String> {
    let out = if has_timeout_bin() {
        Command::new("timeout")
            .args(["3", "bluetoothctl"])
            .args(args)
            .output()
    } else {
        Command::new("bluetoothctl").args(args).output()
    }
    .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Err(format!(
            "exit {}",
            out.status.code().map(|c| c.to_string()).unwrap_or_default()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

/// Short BLE discovery so unpaired devices in range can appear in `bluetoothctl devices` (Jeedom-style).
#[cfg(target_os = "linux")]
fn run_bluetooth_discovery_scan(seconds: u32) -> Result<(), String> {
    if !bluetoothctl_available() {
        return Err("bluetoothctl absent".into());
    }
    if !has_timeout_bin() {
        return Err("binaire timeout(1) introuvable — installez coreutils".into());
    }
    let secs = seconds.clamp(5, 45);
    let outer_secs = secs.saturating_add(12);

    // BlueZ 5.66+ : `bluetoothctl --timeout N scan on` ends after N seconds.
    let r1 = Command::new("timeout")
        .arg(format!("{outer_secs}s"))
        .arg("bluetoothctl")
        .arg("--timeout")
        .arg(secs.to_string())
        .args(["scan", "on"])
        .status();
    if let Ok(st) = r1 {
        if st.success() || st.code() == Some(124) {
            return Ok(());
        }
    }

    // Older bluetoothctl: kill scan after ~secs with GNU timeout (exit 124 = expected).
    let r2 = Command::new("timeout")
        .arg(format!("{}s", secs.saturating_add(4)))
        .arg("bluetoothctl")
        .args(["scan", "on"])
        .status()
        .map_err(|e| e.to_string())?;
    if r2.success() || r2.code() == Some(124) {
        Ok(())
    } else {
        Err(format!(
            "bluetoothctl scan on a échoué (code {:?})",
            r2.code()
        ))
    }
}

#[cfg(target_os = "linux")]
fn parse_bluetoothctl_show(text: &str) -> BluetoothCtlShow {
    let mut s = BluetoothCtlShow::default();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Powered:") {
            s.powered = Some(rest.trim().eq_ignore_ascii_case("yes"));
        } else if let Some(rest) = t.strip_prefix("Discoverable:") {
            s.discoverable = Some(rest.trim().eq_ignore_ascii_case("yes"));
        } else if let Some(rest) = t.strip_prefix("Pairable:") {
            s.pairable = Some(rest.trim().eq_ignore_ascii_case("yes"));
        }
    }
    s
}

/// Lines: `Device AA:BB:CC:DD:EE:FF alias name…`
#[cfg(target_os = "linux")]
fn parse_bluetoothctl_device_lines(text: &str) -> Vec<BluetoothKnownDevice> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("Device ") else {
            continue;
        };
        let rest = rest.trim_start();
        let mut parts = rest.split_whitespace();
        let Some(addr) = parts.next() else {
            continue;
        };
        if !looks_like_bt_mac(addr) {
            continue;
        }
        let name = parts.collect::<Vec<_>>().join(" ");
        out.push(BluetoothKnownDevice {
            address: addr.to_string(),
            name,
            paired: None,
            connected: None,
        });
    }
    out.sort_by(|a, b| a.address.cmp(&b.address));
    out
}

fn looks_like_bt_mac(s: &str) -> bool {
    s.len() == 17 && s.bytes().filter(|&b| b == b':').count() == 5
}

#[cfg(target_os = "linux")]
fn parse_bluetoothctl_device_macs(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| {
            let t = line.trim();
            let rest = t.strip_prefix("Device ")?;
            let addr = rest.split_whitespace().next()?;
            looks_like_bt_mac(addr).then(|| addr.to_ascii_uppercase())
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn list_known_bt_devices(notes: &mut Vec<String>) -> Vec<BluetoothKnownDevice> {
    let Ok(all_text) = run_bt_cmd(&["devices"]) else {
        notes.push("bluetoothctl devices: failed (is the Bluetooth daemon running?)".into());
        return vec![];
    };

    let mut devices = parse_bluetoothctl_device_lines(&all_text);

    let paired: Option<HashSet<String>> = run_bt_cmd(&["devices", "Paired"])
        .ok()
        .map(|t| parse_bluetoothctl_device_macs(&t));
    let connected: Option<HashSet<String>> = run_bt_cmd(&["devices", "Connected"])
        .ok()
        .map(|t| parse_bluetoothctl_device_macs(&t));

    let norm_mac = |m: &str| m.to_ascii_uppercase();
    for d in &mut devices {
        let u = norm_mac(&d.address);
        if let Some(ref p) = paired {
            d.paired = Some(p.contains(&u));
        }
        if let Some(ref c) = connected {
            d.connected = Some(c.contains(&u));
        }
    }

    devices
}

#[cfg(test)]
mod tests {
    use super::normalize_mac;

    #[test]
    fn normalize_mac_colon_upper() {
        assert_eq!(
            normalize_mac("aa:bb:cc:dd:ee:ff").as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
    }

    #[test]
    fn normalize_mac_compact_hex() {
        assert_eq!(
            normalize_mac("aabbccddeeff").as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
    }

    #[test]
    fn normalize_mac_dash() {
        assert_eq!(
            normalize_mac("AA-BB-CC-DD-EE-FF").as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
    }

    #[test]
    fn normalize_mac_rejects_garbage() {
        assert!(normalize_mac("not-a-mac").is_none());
    }

    #[test]
    fn parse_bluetoothctl_info_sample() {
        let t = "Device AA:BB:CC:DD:EE:FF (public)\n\tName: TestPhone\n\tAlias: TestPhone\n\tPaired: yes\n\tConnected: no\n\tRSSI: -58\n\tUUID: Audio Sink (0000110b-...)\n";
        let i = super::parse_bluetoothctl_info("AA:BB:CC:DD:EE:FF", t);
        assert_eq!(i.name.as_deref(), Some("TestPhone"));
        assert_eq!(i.paired, Some(true));
        assert_eq!(i.connected, Some(false));
        assert_eq!(i.rssi, Some(-58));
        assert_eq!(i.uuids.len(), 1);
    }
}
