//! Windows WiFi scanner via Win32 Native WiFi (wlanapi). Reimplements the
//! lswifi flow: WlanScan -> wait -> WlanGetNetworkBssList -> parse BSS + IEs.

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::collections::HashSet;
#[cfg(windows)]
use std::ptr;
#[cfg(windows)]
use wifi_raw::{channel, ie, RawAp, RawError};
#[cfg(not(windows))]
use wifi_raw::{RawAp, RawError};
#[cfg(windows)]
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
#[cfg(windows)]
use windows::Win32::NetworkManagement::WiFi::*;

#[cfg(windows)]
pub fn scan() -> Result<(String, Vec<RawAp>), RawError> {
    unsafe { scan_inner() }
}

#[cfg(not(windows))]
pub fn scan() -> Result<(String, Vec<RawAp>), RawError> {
    Err(RawError::NotImplemented)
}

#[cfg(windows)]
unsafe fn scan_inner() -> Result<(String, Vec<RawAp>), RawError> {
    let mut handle = HANDLE::default();
    let mut negotiated = 0u32;
    if WlanOpenHandle(2, None, &mut negotiated, &mut handle) != ERROR_SUCCESS.0 {
        return Err(RawError::Backend("WlanOpenHandle failed".into()));
    }
    let result = (|| {
        let mut ifaces: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        if WlanEnumInterfaces(handle, None, &mut ifaces) != ERROR_SUCCESS.0 || ifaces.is_null() {
            return Err(RawError::NoInterface);
        }
        if (*ifaces).dwNumberOfItems == 0 {
            WlanFreeMemory(ifaces as *mut c_void);
            return Err(RawError::NoInterface);
        }
        let info = &(*ifaces).InterfaceInfo[0];
        let guid = info.InterfaceGuid;
        let name = String::from_utf16_lossy(&info.strInterfaceDescription)
            .trim_end_matches('\0')
            .to_string();

        let _ = WlanScan(handle, &guid, None, None, None);
        std::thread::sleep(std::time::Duration::from_millis(4000));

        let mut list: *mut WLAN_BSS_LIST = ptr::null_mut();
        let rc = WlanGetNetworkBssList(
            handle,
            &guid,
            None,
            dot11_BSS_type_any,
            false,
            None,
            &mut list,
        );
        if rc == ERROR_SUCCESS.0 && !list.is_null() {
            let aps = parse_bss_list(list);
            WlanFreeMemory(list as *mut c_void);
            WlanFreeMemory(ifaces as *mut c_void);
            return Ok((name, aps));
        }
        // Fallback (no admin / no BSS data): coarse per-SSID list, no band/width.
        let aps = available_networks(handle, &guid);
        WlanFreeMemory(ifaces as *mut c_void);
        Ok((name, aps))
    })();
    WlanCloseHandle(handle, None);
    result
}

#[cfg(windows)]
unsafe fn available_networks(handle: HANDLE, guid: &windows::core::GUID) -> Vec<RawAp> {
    let mut list: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
    if WlanGetAvailableNetworkList(handle, guid, 2, None, &mut list) != ERROR_SUCCESS.0
        || list.is_null()
    {
        return Vec::new();
    }
    let count = (*list).dwNumberOfItems as usize;
    let base = (*list).Network.as_ptr();
    let aps = (0..count)
        .filter_map(|i| {
            let n = &*base.add(i);
            let len = n.dot11Ssid.uSSIDLength as usize;
            if len == 0 {
                return None;
            }
            let ssid = String::from_utf8_lossy(&n.dot11Ssid.ucSSID[..len.min(32)]).to_string();
            Some(RawAp {
                ssid,
                bssid: String::new(),
                channel: 0,
                frequency_mhz: 0,
                center_freq_mhz: 0,
                channel_width_mhz: 20,
                rssi_dbm: (n.wlanSignalQuality as i32 / 2) - 100,
                channel_utilization: None,
                is_dfs: false,
                security: if n.bSecurityEnabled.as_bool() {
                    "secured".into()
                } else {
                    "Open".into()
                },
                phy: "legacy".into(),
            })
        })
        .collect();
    WlanFreeMemory(list as *mut c_void);
    aps
}

#[cfg(windows)]
unsafe fn parse_bss_list(list: *mut WLAN_BSS_LIST) -> Vec<RawAp> {
    let count = (*list).dwNumberOfItems as usize;
    let base = (*list).wlanBssEntries.as_ptr();
    let mut parsed = Vec::new();
    let mut seen = HashSet::new();

    for i in 0..count {
        let entry = parse_entry(&*base.add(i));
        seen.insert(entry.0.bssid.clone());
        parsed.push(entry);
    }

    let mut aps = Vec::new();
    for (ap, _) in &parsed {
        aps.push(ap.clone());
    }

    for (ap, neighbors) in parsed {
        for n in neighbors {
            if seen.insert(n.bssid.clone()) {
                aps.push(RawAp {
                    ssid: ap.ssid.clone(),
                    bssid: n.bssid,
                    channel: n.channel,
                    frequency_mhz: n.frequency_mhz,
                    center_freq_mhz: n.frequency_mhz,
                    channel_width_mhz: n.channel_width_mhz,
                    rssi_dbm: ap.rssi_dbm,
                    channel_utilization: None,
                    is_dfs: false,
                    security: ap.security.clone(),
                    phy: ap.phy.clone(),
                });
            }
        }
    }

    aps
}

#[cfg(windows)]
unsafe fn parse_entry(e: &WLAN_BSS_ENTRY) -> (RawAp, Vec<ie::NeighborAp>) {
    let ssid_len = e.dot11Ssid.uSSIDLength as usize;
    let ssid = String::from_utf8_lossy(&e.dot11Ssid.ucSSID[..ssid_len.min(32)]).to_string();
    let b = e.dot11Bssid;
    let bssid = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5]
    );
    let freq = e.ulChCenterFrequency / 1000;
    let channel = channel::channel_from_freq(freq);

    let ie_ptr = (e as *const WLAN_BSS_ENTRY as *const u8).add(e.ulIeOffset as usize);
    let ies = std::slice::from_raw_parts(ie_ptr, e.ulIeSize as usize);
    let parsed = ie::parse(ies, e.usCapabilityInformation, freq);

    (
        RawAp {
            ssid,
            bssid,
            channel,
            frequency_mhz: freq,
            center_freq_mhz: parsed.center_freq_mhz,
            channel_width_mhz: parsed.channel_width_mhz,
            rssi_dbm: e.lRssi,
            channel_utilization: parsed.utilization,
            is_dfs: channel::is_dfs(freq, channel),
            security: parsed.security,
            phy: parsed.phy,
        },
        parsed.rnr_neighbors,
    )
}
