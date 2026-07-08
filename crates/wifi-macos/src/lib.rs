//! macOS Wi-Fi scanner via CoreWLAN.

#[cfg(target_os = "macos")]
use std::cell::RefCell;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use dispatch2::DispatchQueue;
#[cfg(target_os = "macos")]
use objc2::rc::{autoreleasepool, Retained};
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_core_location::{CLAuthorizationStatus, CLLocationManager};
#[cfg(target_os = "macos")]
use objc2_core_wlan::{
    CWChannel, CWChannelBand, CWChannelWidth, CWNetwork, CWPHYMode, CWSecurity, CWWiFiClient,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSError, NSString};
use wifi_raw::{channel, ie, RawAp, RawError};

#[cfg(target_os = "macos")]
static LOCATION_REQUESTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
thread_local! {
    static LOCATION_MANAGER: RefCell<Option<Retained<CLLocationManager>>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
pub fn scan() -> Result<(String, Vec<RawAp>, Vec<channel::ChannelBand>, Option<String>, bool), RawError> {
    autoreleasepool(|_| unsafe { scan_inner() })
}

#[cfg(not(target_os = "macos"))]
pub fn scan() -> Result<(String, Vec<RawAp>, Vec<channel::ChannelBand>, Option<String>, bool), RawError> {
    Err(RawError::NotImplemented)
}

#[cfg(target_os = "macos")]
pub fn request_location_permission() {
    if MainThreadMarker::new().is_some() {
        autoreleasepool(|_| unsafe { request_location_permission_on_current_thread() });
    } else {
        DispatchQueue::main().exec_async(|| {
            autoreleasepool(|_| unsafe { request_location_permission_on_current_thread() });
        });
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_location_permission() {}

#[cfg(target_os = "macos")]
unsafe fn scan_inner() -> Result<(String, Vec<RawAp>, Vec<channel::ChannelBand>, Option<String>, bool), RawError> {
    let mut permission_pending = false;
    let mut warning = unsafe { location_warning(&mut permission_pending) };

    let client = unsafe { CWWiFiClient::sharedWiFiClient() };
    let interface = unsafe { client.interface() }.ok_or(RawError::NoInterface)?;

    if !unsafe { interface.powerOn() } {
        return Err(RawError::Backend("Wi-Fi interface is powered off".into()));
    }

    let interface_name = string_or(unsafe { interface.interfaceName() }, "Wi-Fi");
    let supported_bands = unsafe { supported_bands(&interface) };
    let networks = unsafe { interface.scanForNetworksWithName_includeHidden_error(None, true) }
        .map_err(|e| RawError::Backend(format!("CoreWLAN scan failed: {}", error_message(&e))))?;
    let mut aps: Vec<RawAp> = networks
        .allObjects()
        .to_vec()
        .iter()
        .filter_map(|network| unsafe { raw_ap_from_network(network) })
        .collect();

    aps.sort_by(|a, b| {
        a.frequency_mhz
            .cmp(&b.frequency_mhz)
            .then_with(|| b.rssi_dbm.cmp(&a.rssi_dbm))
            .then_with(|| a.bssid.cmp(&b.bssid))
    });

    if !aps.is_empty() && aps.iter().all(|ap| ap.ssid == "<hidden>" && ap.bssid.is_empty()) {
        permission_pending = true;
        warning.get_or_insert_with(|| {
            "macOS returned Wi-Fi scan results without SSID/BSSID details. Grant Location permission to WiFi Scanner in System Settings > Privacy & Security > Location Services, then scan again."
                .to_string()
        });
    }

    Ok((interface_name, aps, supported_bands, warning, permission_pending))
}

#[cfg(target_os = "macos")]
unsafe fn location_warning(permission_pending: &mut bool) -> Option<String> {
    if !unsafe { CLLocationManager::locationServicesEnabled_class() } {
        return Some("macOS Location Services are off. Enable Location Services to show Wi-Fi SSIDs and BSSIDs.".into());
    }

    let manager = unsafe { CLLocationManager::new() };
    match unsafe { manager.authorizationStatus() } {
        CLAuthorizationStatus::AuthorizedAlways | CLAuthorizationStatus::AuthorizedWhenInUse => None,
        CLAuthorizationStatus::NotDetermined => {
            request_location_permission();
            *permission_pending = true;
            Some(
                "Location permission is required to show Wi-Fi SSIDs and BSSIDs. Allow WiFi Scanner in the macOS Location prompt, then scan again."
                    .into(),
            )
        }
        CLAuthorizationStatus::Denied | CLAuthorizationStatus::Restricted => Some(
            "Location permission is denied for WiFi Scanner. Enable it in System Settings > Privacy & Security > Location Services, then scan again."
                .into(),
        ),
        _ => Some(
            "Location permission is required to show Wi-Fi SSIDs and BSSIDs.".into(),
        ),
    }
}

#[cfg(target_os = "macos")]
unsafe fn request_location_permission_on_current_thread() {
    if !unsafe { CLLocationManager::locationServicesEnabled_class() } {
        return;
    }

    LOCATION_MANAGER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let manager = slot.get_or_insert_with(|| unsafe { CLLocationManager::new() });
        if unsafe { manager.authorizationStatus() } != CLAuthorizationStatus::NotDetermined
            || LOCATION_REQUESTED.swap(true, Ordering::SeqCst)
        {
            return;
        }
        unsafe { manager.requestWhenInUseAuthorization() };
    });
}

#[cfg(target_os = "macos")]
unsafe fn supported_bands(interface: &objc2_core_wlan::CWInterface) -> Vec<channel::ChannelBand> {
    let mut bands = Vec::new();
    if let Some(channels) = unsafe { interface.supportedWLANChannels() } {
        for ch in channels.allObjects().to_vec() {
            let band = unsafe { channel_band(&ch) };
            if band != channel::ChannelBand::Unknown && !bands.contains(&band) {
                bands.push(band);
            }
        }
    }
    if bands.is_empty() {
        bands.extend([channel::ChannelBand::Band2_4, channel::ChannelBand::Band5]);
    }
    bands
}

#[cfg(target_os = "macos")]
unsafe fn raw_ap_from_network(network: &CWNetwork) -> Option<RawAp> {
    let rssi = unsafe { network.rssiValue() } as i32;
    if !(-120..0).contains(&rssi) {
        return None;
    }

    let ssid = ssid(network);
    let bssid = string_opt(unsafe { network.bssid() })
        .unwrap_or_default()
        .to_ascii_lowercase();
    let channel_ref = unsafe { network.wlanChannel() };
    let channel_number = channel_ref
        .as_deref()
        .map(|c| unsafe { c.channelNumber() } as u16)
        .unwrap_or(0);
    let band = channel_ref
        .as_deref()
        .map(|c| unsafe { channel_band(c) })
        .unwrap_or(channel::ChannelBand::Unknown);
    let frequency_mhz = channel::freq_from_channel(channel_number, band);
    let width_from_channel = channel_ref
        .as_deref()
        .map(|c| unsafe { channel_width(c) })
        .unwrap_or(20);
    let ies = unsafe { network.informationElementData() }
        .map(|data| data.to_vec())
        .unwrap_or_default();
    let parsed = ie::parse(&ies, 0, frequency_mhz);
    let channel_width_mhz = width_from_channel.max(parsed.channel_width_mhz);
    let center_freq_mhz = if parsed.center_freq_mhz > 0 {
        parsed.center_freq_mhz
    } else {
        frequency_mhz
    };
    let phy = phy(network, &parsed.phy);

    Some(RawAp {
        ssid,
        bssid,
        channel: channel_number,
        frequency_mhz,
        center_freq_mhz,
        channel_width_mhz,
        rssi_dbm: rssi,
        channel_utilization: parsed.utilization,
        is_dfs: channel::is_dfs(frequency_mhz, channel_number),
        security: security(network, &parsed.security),
        phy,
    })
}

#[cfg(target_os = "macos")]
unsafe fn ssid(network: &CWNetwork) -> String {
    if let Some(ssid) = string_opt(unsafe { network.ssid() }) {
        if !ssid.is_empty() {
            return ssid;
        }
    }

    unsafe { network.ssidData() }
        .map(|data| String::from_utf8_lossy(&data.to_vec()).to_string())
        .filter(|ssid| !ssid.is_empty())
        .unwrap_or_else(|| "<hidden>".to_string())
}

#[cfg(target_os = "macos")]
unsafe fn channel_band(channel_ref: &CWChannel) -> channel::ChannelBand {
    match unsafe { channel_ref.channelBand() } {
        CWChannelBand::Band2GHz => channel::ChannelBand::Band2_4,
        CWChannelBand::Band5GHz => channel::ChannelBand::Band5,
        CWChannelBand::Band6GHz => channel::ChannelBand::Band6,
        _ => channel::ChannelBand::Unknown,
    }
}

#[cfg(target_os = "macos")]
unsafe fn channel_width(channel_ref: &CWChannel) -> u16 {
    match unsafe { channel_ref.channelWidth() } {
        CWChannelWidth::Width40MHz => 40,
        CWChannelWidth::Width80MHz => 80,
        CWChannelWidth::Width160MHz => 160,
        _ => 20,
    }
}

#[cfg(target_os = "macos")]
unsafe fn security(network: &CWNetwork, parsed_security: &str) -> String {
    if unsafe { network.supportsSecurity(CWSecurity::WPA3Personal) }
        || unsafe { network.supportsSecurity(CWSecurity::WPA3Enterprise) }
        || unsafe { network.supportsSecurity(CWSecurity::WPA3Transition) }
    {
        "WPA3".to_string()
    } else if unsafe { network.supportsSecurity(CWSecurity::WPA2Personal) }
        || unsafe { network.supportsSecurity(CWSecurity::WPA2Enterprise) }
        || unsafe { network.supportsSecurity(CWSecurity::Personal) }
        || unsafe { network.supportsSecurity(CWSecurity::Enterprise) }
    {
        "WPA2".to_string()
    } else if unsafe { network.supportsSecurity(CWSecurity::WPAPersonal) }
        || unsafe { network.supportsSecurity(CWSecurity::WPAEnterprise) }
        || unsafe { network.supportsSecurity(CWSecurity::WPAPersonalMixed) }
        || unsafe { network.supportsSecurity(CWSecurity::WPAEnterpriseMixed) }
    {
        "WPA".to_string()
    } else if unsafe { network.supportsSecurity(CWSecurity::OWE) }
        || unsafe { network.supportsSecurity(CWSecurity::OWETransition) }
    {
        "OWE".to_string()
    } else if unsafe { network.supportsSecurity(CWSecurity::WEP) }
        || unsafe { network.supportsSecurity(CWSecurity::DynamicWEP) }
    {
        "WEP".to_string()
    } else if parsed_security != "Open" {
        parsed_security.to_string()
    } else {
        "Open".to_string()
    }
}

#[cfg(target_os = "macos")]
unsafe fn phy(network: &CWNetwork, parsed_phy: &str) -> String {
    if parsed_phy == "be" {
        return "be".to_string();
    }

    if unsafe { network.supportsPHYMode(CWPHYMode::Mode11ax) } {
        "ax".to_string()
    } else if unsafe { network.supportsPHYMode(CWPHYMode::Mode11ac) } {
        "ac".to_string()
    } else if unsafe { network.supportsPHYMode(CWPHYMode::Mode11n) } {
        "n".to_string()
    } else if unsafe { network.supportsPHYMode(CWPHYMode::Mode11g) } {
        "g".to_string()
    } else if unsafe { network.supportsPHYMode(CWPHYMode::Mode11b) } {
        "b".to_string()
    } else if unsafe { network.supportsPHYMode(CWPHYMode::Mode11a) } {
        "a".to_string()
    } else {
        parsed_phy.to_string()
    }
}

#[cfg(target_os = "macos")]
fn string_opt(value: Option<Retained<NSString>>) -> Option<String> {
    value.map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
fn string_or(value: Option<Retained<NSString>>, fallback: &str) -> String {
    string_opt(value)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(target_os = "macos")]
fn error_message(error: &NSError) -> String {
    format!("{} (code {})", error.localizedDescription(), error.code())
}
