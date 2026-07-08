use serde::Serialize;
use thiserror::Error;

/// WiFi frequency band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    Band2_4,
    Band5,
    Band6,
    Unknown,
}

impl Band {
    /// Classify a center frequency (MHz) into a band.
    pub fn from_freq_mhz(freq: u32) -> Band {
        match freq {
            2400..=2500 => Band::Band2_4,
            4900..=5895 => Band::Band5,
            5925..=7125 => Band::Band6,
            _ => Band::Unknown,
        }
    }
}

/// A single scanned access point (BSS).
#[derive(Debug, Clone, Serialize)]
pub struct AccessPoint {
    /// Stable unique id for this BSS (BSSID, or synth when unavailable).
    pub id: String,
    pub ssid: String,
    pub bssid: String,
    pub band: Band,
    pub channel: u16,
    pub frequency_mhz: u32,
    pub center_freq_mhz: u32,
    pub channel_width_mhz: u16,
    pub rssi_dbm: i32,
    /// QBSS channel utilization 0-255 (congestion), if advertised.
    pub channel_utilization: Option<u8>,
    pub is_dfs: bool,
    pub security: String,
    pub phy: String,
}

/// Result of one scan pass.
#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub access_points: Vec<AccessPoint>,
    pub interface: String,
    pub supported_bands: Vec<Band>,
    pub warning: Option<String>,
    pub retry_after_permission_change: bool,
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("wifi scanning not implemented on this platform")]
    NotImplemented,
    #[error("no wireless interface found")]
    NoInterface,
    #[error("scan failed: {0}")]
    Backend(String),
}
