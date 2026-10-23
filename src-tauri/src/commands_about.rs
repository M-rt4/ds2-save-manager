//! What the About view shows about the machine the app runs on.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// e.g. "Windows 11 Pro 26H2 (build 26300.9711)".
    os: Option<String>,
    arch: &'static str,
}

#[tauri::command]
pub async fn system_info() -> SystemInfo {
    SystemInfo { os: windows_version(), arch: std::env::consts::ARCH }
}

#[cfg(windows)]
fn windows_version() -> Option<String> {
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

#[cfg(not(windows))]
fn windows_version() -> Option<String> {
    None
}
