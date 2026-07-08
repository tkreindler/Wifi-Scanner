use wifi_core::ScanResult;

#[tauri::command]
fn scan() -> Result<ScanResult, String> {
    let mut s = wifi_core::create_scanner().map_err(|e| e.to_string())?;
    s.scan().map_err(|e| e.to_string())
}

#[tauri::command]
fn request_permissions() {
    wifi_core::request_platform_permissions();
}

/// WiFi BSS scanning needs admin; relaunch elevated if needed (single UAC prompt).
#[cfg(windows)]
fn ensure_admin() {
    use windows::core::w;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    if is_elevated() || std::env::var("WIFI_SCANNER_NOELEVATE").is_ok() {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let exe: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
        unsafe {
            ShellExecuteW(None, w!("runas"), windows::core::PCWSTR(exe.as_ptr()), None, None, SW_SHOWNORMAL);
        }
        std::process::exit(0);
    }
}

#[cfg(windows)]
fn is_elevated() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elev = TOKEN_ELEVATION::default();
        let mut sz = 0u32;
        let ok = GetTokenInformation(token, TokenElevation, Some(&mut elev as *mut _ as _), std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut sz).is_ok();
        CloseHandle(token).ok();
        ok && elev.TokenIsElevated != 0
    }
}

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    ensure_admin();
    tauri::Builder::default()
        .setup(|_| {
            wifi_core::request_platform_permissions();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![scan, request_permissions])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
