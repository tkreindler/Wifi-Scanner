use crate::model::{Band, ScanError, ScanResult};
use crate::scanner::WifiScanner;
use wifi_raw::RawAp;

/// Mock scanner for tests/UI when WIFI_SCANNER_MOCK=1 or no real APs are reachable.
pub struct MockScanner;

impl WifiScanner for MockScanner {
    fn scan(&mut self) -> Result<ScanResult, ScanError> {
        let raws = sample();
        Ok(ScanResult { access_points: raws.into_iter().map(crate::scanner::convert).collect(), interface: "Mock Adapter".into(), supported_bands: vec![Band::Band2_4, Band::Band5, Band::Band6], warning: None, retry_after_permission_change: false })
    }
}

fn ap(ssid: &str, bssid: &str, freq: u32, ch: u16, w: u16, rssi: i32, util: u8, dfs: bool) -> RawAp {
    RawAp { ssid: ssid.into(), bssid: bssid.into(), channel: ch, frequency_mhz: freq, center_freq_mhz: freq, channel_width_mhz: w, rssi_dbm: rssi, channel_utilization: Some(util), is_dfs: dfs, security: "WPA2".into(), phy: "ax".into() }
}

fn sample() -> Vec<RawAp> {
    vec![
        ap("Home-2G", "aa:bb:cc:00:00:01", 2412, 1, 20, -45, 30, false),
        ap("Office", "aa:bb:cc:00:00:02", 2437, 6, 20, -67, 75, false),
        ap("Home-5G", "aa:bb:cc:00:00:03", 5180, 36, 80, -52, 20, false),
        ap("Radar5", "aa:bb:cc:00:00:04", 5500, 100, 40, -71, 55, true),
        ap("Mesh6E", "aa:bb:cc:00:00:05", 5975, 5, 160, -58, 10, false),
        ap("WiFi7", "aa:bb:cc:00:00:06", 6135, 37, 320, -61, 40, false),
    ]
}

pub fn band_summary(r: &ScanResult) -> Vec<(Band, usize)> {
    use Band::*;
    [Band2_4, Band5, Band6].iter().map(|b| (*b, r.access_points.iter().filter(|a| a.band == *b).count())).collect()
}
