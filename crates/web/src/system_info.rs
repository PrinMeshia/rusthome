//! Host and resource snapshot for the `/system` dashboard (read-only, lab UI).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use sysinfo::{Components, Disk, Disks, System};

/// USB serial adapter seen under `/dev` (Conbee and similar dongles often appear as `ttyACM*` / `ttyUSB*`).
#[derive(Debug, Clone, Serialize)]
pub struct SerialPortInfo {
    pub device: String,
    /// Stable name under `/dev/serial/by-id/` when available.
    pub by_id_name: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub product_label: Option<String>,
    /// Heuristic: Dresden Elektronik USB vendor id (Conbee family).
    pub maybe_conbee_hint: bool,
    pub notes: Vec<String>,
}

/// Serializable snapshot for `/api/system` and the system HTML page.
#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub rusthome_version: String,
    pub service: String,
    pub listen: String,
    pub data_dir: String,
    pub journal_path: String,
    pub journal_file_bytes: Option<u64>,
    pub journal_file_present: bool,
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub os_long: Option<String>,
    pub kernel: Option<String>,
    pub cpu_arch: String,
    pub uptime_secs: u64,
    pub load_avg_1: f64,
    pub load_avg_5: f64,
    pub load_avg_15: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub cpu_count: usize,
    pub cpu_usage_percent: f32,
    /// Max reported temperature among hardware sensors (°C), if any.
    pub cpu_temp_c_max: Option<f32>,
    pub disk_mount: Option<String>,
    pub disk_total_bytes: Option<u64>,
    pub disk_available_bytes: Option<u64>,
    /// `ttyACM*` / `ttyUSB*` on Unix (best-effort udev metadata on Linux).
    pub serial_ports: Vec<SerialPortInfo>,
}

pub fn capture(listen: &str, data_dir: &Path, journal_path: &Path) -> SystemSnapshot {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();

    let load = System::load_average();
    let journal_file_present = journal_path.is_file();
    let journal_file_bytes = std::fs::metadata(journal_path).ok().map(|m| m.len());

    let canonical_data = std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    let disk = best_disk_for_path(&canonical_data);

    let mut temp_max: Option<f32> = None;
    let components = Components::new_with_refreshed_list();
    for c in components.iter() {
        if let Some(t) = c.temperature() {
            temp_max = Some(temp_max.map_or(t, |m| m.max(t)));
        }
    }

    let (disk_mount, disk_total_bytes, disk_available_bytes) = match disk {
        Some((m, t, a)) => (Some(m), Some(t), Some(a)),
        None => (None, None, None),
    };

    let serial_ports = list_serial_usb_devices();

    SystemSnapshot {
        rusthome_version: env!("CARGO_PKG_VERSION").to_string(),
        service: "rusthome-web".to_string(),
        listen: listen.to_string(),
        data_dir: data_dir.display().to_string(),
        journal_path: journal_path.display().to_string(),
        journal_file_bytes,
        journal_file_present,
        hostname: System::host_name(),
        os_name: System::name(),
        os_long: System::long_os_version(),
        kernel: System::kernel_version(),
        cpu_arch: System::cpu_arch(),
        uptime_secs: System::uptime(),
        load_avg_1: load.one,
        load_avg_5: load.five,
        load_avg_15: load.fifteen,
        memory_total_bytes: sys.total_memory(),
        memory_used_bytes: sys.used_memory(),
        swap_total_bytes: sys.total_swap(),
        swap_used_bytes: sys.used_swap(),
        cpu_count: sys.cpus().len(),
        cpu_usage_percent: sys.global_cpu_usage(),
        cpu_temp_c_max: temp_max,
        disk_mount,
        disk_total_bytes,
        disk_available_bytes,
        serial_ports,
    }
}

/// Lists `/dev/ttyACM*` and `/dev/ttyUSB*` with optional udev metadata (Linux).
pub fn list_serial_usb_devices() -> Vec<SerialPortInfo> {
    #[cfg(unix)]
    {
        list_serial_usb_devices_unix()
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

#[cfg(unix)]
fn list_serial_usb_devices_unix() -> Vec<SerialPortInfo> {
    let mut names: Vec<String> = Vec::new();
    let Ok(rd) = std::fs::read_dir("/dev") else {
        return Vec::new();
    };
    for e in rd.filter_map(|x| x.ok()) {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("ttyACM") || name.starts_with("ttyUSB") {
            names.push(name);
        }
    }
    names.sort();
    names
        .into_iter()
        .map(|n| probe_serial_port(&format!("/dev/{n}")))
        .collect()
}

#[cfg(unix)]
fn probe_serial_port(device: &str) -> SerialPortInfo {
    let mut notes = Vec::new();
    let path = PathBuf::from(device);
    let by_id_name = find_serial_by_id(&path);
    #[cfg(target_os = "linux")]
    let (vendor_id, product_id, product_label) = udev_properties_linux(device, &mut notes);
    #[cfg(not(target_os = "linux"))]
    let (vendor_id, product_id, product_label) = (None, None, None);

    let maybe_conbee_hint = vendor_id
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("1cf1"))
        .unwrap_or(false);

    SerialPortInfo {
        device: device.to_string(),
        by_id_name,
        vendor_id,
        product_id,
        product_label,
        maybe_conbee_hint,
        notes,
    }
}

#[cfg(unix)]
fn find_serial_by_id(dev: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(dev).ok()?;
    let by_id = Path::new("/dev/serial/by-id");
    let Ok(rd) = std::fs::read_dir(by_id) else {
        return None;
    };
    for e in rd.filter_map(|x| x.ok()) {
        let p = e.path();
        if let Ok(t) = std::fs::canonicalize(&p) {
            if t == canonical {
                return Some(e.file_name().to_string_lossy().into());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn udev_properties_linux(device: &str, notes: &mut Vec<String>) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(out) = Command::new("udevadm")
        .args(["info", "--query=property", &format!("--name={device}")])
        .output()
    else {
        notes.push("udevadm indisponible ou échec".into());
        return (None, None, None);
    };
    if !out.status.success() {
        notes.push(format!(
            "udevadm status {}",
            out.status.code().unwrap_or(-1)
        ));
        return (None, None, None);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut m: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            m.insert(k.to_string(), v.to_string());
        }
    }
    let vendor_id = m.get("ID_VENDOR_ID").cloned().or_else(|| m.get("ID_USB_VENDOR_ID").cloned());
    let product_id = m.get("ID_MODEL_ID").cloned().or_else(|| m.get("ID_USB_MODEL_ID").cloned());
    let product_label = m
        .get("ID_MODEL_FROM_DATABASE")
        .cloned()
        .or_else(|| m.get("ID_MODEL").cloned());
    (vendor_id, product_id, product_label)
}

fn best_disk_for_path(path: &Path) -> Option<(String, u64, u64)> {
    let disks = Disks::new_with_refreshed_list();
    let mut best: Option<(&Disk, usize)> = None;
    for disk in disks.list() {
        let mp = disk.mount_point();
        if path.starts_with(mp) {
            let len = mp.as_os_str().len();
            if best.map(|(_, l)| l).unwrap_or(0) < len {
                best = Some((disk, len));
            }
        }
    }
    best.map(|(d, _)| {
        (
            d.mount_point().display().to_string(),
            d.total_space(),
            d.available_space(),
        )
    })
}

/// Used by `lib.rs` for HTML escaping.
pub fn fmt_bytes(n: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let x = n as f64;
    if x >= GB {
        format!("{:.2} GiB", x / GB)
    } else if x >= MB {
        format!("{:.1} MiB", x / MB)
    } else if x >= 1024.0 {
        format!("{:.1} KiB", x / 1024.0)
    } else {
        format!("{n} B")
    }
}

pub fn fmt_duration(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

pub fn opt_str(o: &Option<String>) -> String {
    o.as_deref().unwrap_or("—").to_string()
}
