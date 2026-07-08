//! TARGET 1: E2E functional tests against the abstraction layer. On Windows the
//! Windows impl runs internally. Verifies real scanning works and the mock path
//! exercises all bands incl. DFS/6 GHz.
use wifi_core::{band_summary, create_scanner, Band};

#[test]
fn real_scan_returns_ok_with_interface() {
    std::env::remove_var("WIFI_SCANNER_MOCK");
    let mut s = create_scanner().expect("scanner");
    let r = s.scan().expect("scan should succeed (may be empty if no APs)");
    assert!(!r.interface.is_empty(), "interface name should be reported");
    for ap in &r.access_points {
        assert!(ap.rssi_dbm < 0 && ap.rssi_dbm > -120);
    }
}

#[test]
fn mock_covers_all_bands_dfs_and_congestion() {
    std::env::set_var("WIFI_SCANNER_MOCK", "1");
    let r = create_scanner().unwrap().scan().unwrap();
    let bands = band_summary(&r);
    for (b, n) in bands {
        assert!(n > 0, "expected APs in band {b:?}");
    }
    assert!(r.access_points.iter().any(|a| a.is_dfs), "expected a DFS network");
    assert!(r.access_points.iter().any(|a| a.band == Band::Band6 && a.channel_width_mhz >= 160));
    assert!(r.access_points.iter().all(|a| a.channel_utilization.is_some()));
    std::env::remove_var("WIFI_SCANNER_MOCK");
}
