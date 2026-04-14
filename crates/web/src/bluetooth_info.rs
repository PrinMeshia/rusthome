//! Read-only Bluetooth inventory for the system dashboard (Linux: sysfs + optional `bluetoothctl`).
//!
//! Scanning, pairing, and GATT are **not** implemented here — those need BlueZ/D-Bus integrations
//! and stronger security boundaries than this lab UI.

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

#[derive(Default)]
struct BluetoothCtlShow {
    powered: Option<bool>,
    discoverable: Option<bool>,
    pairable: Option<bool>,
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

#[cfg(target_os = "linux")]
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
