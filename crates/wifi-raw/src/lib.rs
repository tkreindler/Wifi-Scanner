//! Shared contract between `wifi-core` and the platform implementations.
//! Leaf crate with no dependencies on either, so there is no dependency cycle.

use thiserror::Error;

pub mod channel;
pub mod ie;

/// Platform-agnostic raw access point. `wifi-core` maps this to its public model.
#[derive(Debug, Clone)]
pub struct RawAp {
    pub ssid: String,
    pub bssid: String,
    pub channel: u16,
    pub frequency_mhz: u32,
    pub center_freq_mhz: u32,
    pub channel_width_mhz: u16,
    pub rssi_dbm: i32,
    pub channel_utilization: Option<u8>,
    pub is_dfs: bool,
    pub security: String,
    pub phy: String,
}

#[derive(Debug, Error)]
pub enum RawError {
    #[error("not implemented")]
    NotImplemented,
    #[error("no wireless interface")]
    NoInterface,
    #[error("{0}")]
    Backend(String),
}
