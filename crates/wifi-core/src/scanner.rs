use crate::model::{AccessPoint, Band, ScanError, ScanResult};

/// Cross-platform scanner interface. The Tauri app depends only on this.
pub trait WifiScanner: Send {
    /// Trigger a scan and return all visible access points.
    fn scan(&mut self) -> Result<ScanResult, ScanError>;
}

/// Construct the platform scanner. Windows uses Native WiFi, macOS uses
/// CoreWLAN, and other platforms are unsupported.
pub fn create_scanner() -> Result<Box<dyn WifiScanner>, ScanError> {
    if std::env::var("WIFI_SCANNER_MOCK").as_deref() == Ok("1") {
        return Ok(Box::new(crate::mock::MockScanner));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(WindowsScanner))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacScanner))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(ScanError::NotImplemented)
    }
}

/// Ask the current platform for permissions needed before a scan can show full details.
pub fn request_platform_permissions() {
    #[cfg(target_os = "macos")]
    wifi_macos::request_location_permission();
}

#[cfg(windows)]
struct WindowsScanner;

#[cfg(windows)]
impl WifiScanner for WindowsScanner {
    fn scan(&mut self) -> Result<ScanResult, ScanError> {
        let (interface, raws) = wifi_windows::scan().map_err(map_err)?;
        Ok(ScanResult { access_points: raws.into_iter().map(convert).collect(), interface, supported_bands: vec![Band::Band2_4, Band::Band5, Band::Band6], warning: None, retry_after_permission_change: false })
    }
}

#[cfg(target_os = "macos")]
struct MacScanner;

#[cfg(target_os = "macos")]
impl WifiScanner for MacScanner {
    fn scan(&mut self) -> Result<ScanResult, ScanError> {
        let (interface, raws, supported_bands, warning, retry_after_permission_change) = wifi_macos::scan().map_err(map_err)?;
        Ok(ScanResult { access_points: raws.into_iter().map(convert).collect(), interface, supported_bands: supported_bands.into_iter().map(convert_band).collect(), warning, retry_after_permission_change })
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn map_err(e: wifi_raw::RawError) -> ScanError {
    match e {
        wifi_raw::RawError::NotImplemented => ScanError::NotImplemented,
        wifi_raw::RawError::NoInterface => ScanError::NoInterface,
        wifi_raw::RawError::Backend(m) => ScanError::Backend(m),
    }
}

pub(crate) fn convert(r: wifi_raw::RawAp) -> AccessPoint {
    AccessPoint {
        id: if r.bssid.is_empty() { format!("{}-{}-{}", r.ssid, r.channel, r.channel_width_mhz) } else { r.bssid.clone() },
        ssid: r.ssid,
        bssid: r.bssid,
        band: Band::from_freq_mhz(r.frequency_mhz),
        channel: r.channel,
        frequency_mhz: r.frequency_mhz,
        center_freq_mhz: if r.center_freq_mhz > 0 { r.center_freq_mhz } else { r.frequency_mhz },
        channel_width_mhz: r.channel_width_mhz,
        rssi_dbm: r.rssi_dbm,
        channel_utilization: r.channel_utilization,
        is_dfs: r.is_dfs,
        security: r.security,
        phy: r.phy,
    }
}

#[cfg(target_os = "macos")]
fn convert_band(band: wifi_raw::channel::ChannelBand) -> Band {
    match band {
        wifi_raw::channel::ChannelBand::Band2_4 => Band::Band2_4,
        wifi_raw::channel::ChannelBand::Band5 => Band::Band5,
        wifi_raw::channel::ChannelBand::Band6 => Band::Band6,
        wifi_raw::channel::ChannelBand::Unknown => Band::Unknown,
    }
}
