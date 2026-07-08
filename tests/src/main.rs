fn main() {
    match wifi_core::create_scanner().and_then(|mut s| s.scan()) {
        Ok(r) => {
            if std::env::args().any(|a| a == "--json") {
                println!("{}", serde_json::to_string(&r).unwrap());
                return;
            }
            println!("interface: {}", r.interface);
            for ap in r.access_points {
                println!("{:?} {} ch{} {}MHz {}dBm {:?}", ap.band, ap.ssid, ap.channel, ap.frequency_mhz, ap.rssi_dbm, ap.channel_utilization);
            }
        }
        Err(e) => eprintln!("scan error: {e}"),
    }
}
