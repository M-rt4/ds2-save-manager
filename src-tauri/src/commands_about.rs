//! What the About view shows about the machine the app runs on.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// e.g. "Windows 11 Pro 26H2 (build 26300.9711)" or "SteamOS 3.7 (Steam Deck OLED)".
    os: Option<String>,
    arch: &'static str,
}

#[tauri::command]
pub async fn system_info() -> SystemInfo {
    SystemInfo { os: os_version(), arch: std::env::consts::ARCH }
}

#[cfg(windows)]
fn os_version() -> Option<String> {
    use std::fmt::Write;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion").ok()?;
    let mut product: String = key.get_value("ProductName").ok()?;
    let build: String = key.get_value("CurrentBuild").unwrap_or_default();
    // Windows 11 still names itself "Windows 10" here; its builds start at 22000.
    if build.parse::<u32>().is_ok_and(|b| b >= 22000) {
        product = product.replacen("Windows 10", "Windows 11", 1);
    }
    if let Ok(release) = key.get_value::<String, _>("DisplayVersion") {
        let _ = write!(product, " {release}");
    }
    if !build.is_empty() {
        let revision = key.get_value::<u32, _>("UBR").map(|u| format!(".{u}")).unwrap_or_default();
        let _ = write!(product, " (build {build}{revision})");
    }
    Some(product)
}

/// The distribution's own name from os-release, the kernel, and the device when it is a
/// Steam Deck (Valve's boards are "Jupiter" for the LCD model and "Galileo" for the OLED).
#[cfg(not(windows))]
fn os_version() -> Option<String> {
    let release = std::fs::read_to_string("/etc/os-release").ok()?;
    let field = |key: &str| {
        release.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=').map(|v| v.trim_matches('"').to_string()))
    };
    let mut name = field("PRETTY_NAME").or_else(|| field("NAME"))?;
    let dmi = |f: &str| std::fs::read_to_string(format!("/sys/class/dmi/id/{f}")).map(|s| s.trim().to_string()).unwrap_or_default();
    if dmi("board_vendor") == "Valve" {
        let model = match dmi("board_name").as_str() {
            "Galileo" => "Steam Deck OLED",
            "Jupiter" => "Steam Deck",
            _ => "Valve",
        };
        name = format!("{name} ({model})");
    }
    if let Ok(kernel) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        name = format!("{name}, Linux {}", kernel.trim());
    }
    Some(name)
}

/// Development copies name themselves (`DS2SM_DEV_LABEL`, e.g. "worker1 · gamepad") so the
/// user can tell the copies on screen apart; release builds have no label.
#[tauri::command]
pub fn dev_label() -> Option<String> {
    cfg!(debug_assertions).then(|| std::env::var("DS2SM_DEV_LABEL").unwrap_or_else(|_| "dev".into()))
}

/// The scratch saves folder a test copy was pointed at (`DS2SM_SAVES_ROOT`), so the window can
/// say plainly that the real saves are not the ones it lists and writes.
#[tauri::command]
pub fn test_saves_root() -> Option<String> {
    std::env::var("DS2SM_SAVES_ROOT").ok().filter(|r| !r.is_empty())
}
