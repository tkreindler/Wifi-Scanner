//! 802.11 Information Element parsing for width, congestion, security, and PHY.

use crate::channel::{freq_from_channel, ChannelBand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighborAp {
    pub bssid: String,
    pub channel: u16,
    pub frequency_mhz: u32,
    pub channel_width_mhz: u16,
}

pub struct ParsedIes {
    pub channel_width_mhz: u16,
    pub center_freq_mhz: u32,
    pub utilization: Option<u8>,
    pub security: String,
    pub phy: String,
    pub rnr_neighbors: Vec<NeighborAp>,
}

pub fn parse(ies: &[u8], cap_info: u16, primary_freq: u32) -> ParsedIes {
    let mut width = 20u16;
    let mut center = primary_freq;
    let mut util = None;
    let mut phy = "legacy";
    let mut has_rsn = false;
    let mut akm_sae = false;
    let mut has_wpa1 = false;
    let mut rnr_neighbors = Vec::new();

    let mut i = 0usize;
    while i + 2 <= ies.len() {
        let id = ies[i];
        let len = ies[i + 1] as usize;
        let body_start = i + 2;
        if body_start + len > ies.len() {
            break;
        }
        let body = &ies[body_start..body_start + len];
        match id {
            11 if len >= 3 => {
                util = Some((((body[2] as f64) / 255.0 * 100.0).ceil()) as u8);
            }
            45 => phy = "n",
            48 => {
                has_rsn = true;
                parse_rsn(body, &mut akm_sae);
            }
            61 if len >= 2 => {
                let sco = body[1] & 0x03;
                if sco == 1 || sco == 3 {
                    width = width.max(40);
                    center = if sco == 1 {
                        primary_freq + 10
                    } else {
                        primary_freq - 10
                    };
                }
            }
            191 => phy = "ac",
            192 if len >= 3 => {
                let cw = body[0];
                let ccfs0 = body[1] as u32;
                let ccfs1 = body[2] as u32;
                if cw >= 1 {
                    width = width.max(80);
                    if ccfs0 > 0 {
                        center = 5000 + ccfs0 * 5;
                    }
                    if ccfs1 > 0 {
                        width = if (ccfs1 as i32 - ccfs0 as i32).abs() == 8 {
                            160
                        } else {
                            80
                        };
                    }
                }
            }
            201 => parse_rnr(body, &mut rnr_neighbors),
            221 if len >= 4 && body[..4] == [0x00, 0x50, 0xF2, 0x01] => has_wpa1 = true,
            255 if !body.is_empty() => match body[0] {
                36 => phy = "ax",
                106 => phy = "be",
                _ => {}
            },
            _ => {}
        }
        i = body_start + len;
    }

    let security = if akm_sae {
        "WPA3".to_string()
    } else if has_rsn {
        "WPA2".to_string()
    } else if has_wpa1 {
        "WPA".to_string()
    } else if cap_info & 0x0010 != 0 {
        "WEP".to_string()
    } else {
        "Open".to_string()
    };

    ParsedIes {
        channel_width_mhz: width,
        center_freq_mhz: center,
        utilization: util,
        security,
        phy: phy.to_string(),
        rnr_neighbors,
    }
}

fn parse_rnr(body: &[u8], out: &mut Vec<NeighborAp>) {
    let mut i = 0usize;
    while i + 4 <= body.len() {
        let header = body[i];
        let tbtt_count = ((header >> 4) as usize) + 1;
        let tbtt_len = body[i + 1] as usize;
        let operating_class = body[i + 2];
        let channel = body[i + 3] as u16;
        i += 4;

        let width = match operating_class {
            131 | 136 => 20,
            132 => 40,
            133 => 80,
            134 => 160,
            135 => 80,
            _ => {
                i = i.saturating_add(tbtt_count.saturating_mul(tbtt_len));
                continue;
            }
        };
        let frequency = freq_from_channel(channel, ChannelBand::Band6);
        if frequency == 0 || tbtt_len == 0 {
            i = i.saturating_add(tbtt_count.saturating_mul(tbtt_len));
            continue;
        }

        for _ in 0..tbtt_count {
            if i + tbtt_len > body.len() {
                return;
            }
            let tbtt = &body[i..i + tbtt_len];
            if tbtt.len() >= 7 {
                let b = &tbtt[1..7];
                out.push(NeighborAp {
                    bssid: format!(
                        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                        b[0], b[1], b[2], b[3], b[4], b[5]
                    ),
                    channel,
                    frequency_mhz: frequency,
                    channel_width_mhz: width,
                });
            }
            i += tbtt_len;
        }
    }
}

fn parse_rsn(body: &[u8], sae: &mut bool) {
    if body.len() < 8 {
        return;
    }
    let pairwise = u16::from_le_bytes([body[6], body[7]]) as usize;
    let akm_off = 8 + pairwise * 4;
    if akm_off + 2 > body.len() {
        return;
    }
    let akm_count = u16::from_le_bytes([body[akm_off], body[akm_off + 1]]) as usize;
    for n in 0..akm_count {
        let o = akm_off + 2 + n * 4;
        if o + 4 > body.len() {
            break;
        }
        if matches!(body[o + 3], 8 | 9) {
            *sae = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qbss_utilization_as_percent() {
        let parsed = parse(&[11, 3, 0, 0, 128], 0, 2412);

        assert_eq!(parsed.utilization, Some(51));
    }

    #[test]
    fn parses_vht_width_and_center_frequency() {
        let parsed = parse(&[192, 3, 1, 42, 0], 0, 5180);

        assert_eq!(parsed.channel_width_mhz, 80);
        assert_eq!(parsed.center_freq_mhz, 5210);
    }

    #[test]
    fn parses_wpa3_sae_from_rsn_akm() {
        let parsed = parse(
            &[
                48, 18, 1, 0, 0, 15, 172, 4, 1, 0, 0, 15, 172, 4, 1, 0, 0, 15, 172, 8,
            ],
            0,
            2412,
        );

        assert_eq!(parsed.security, "WPA3");
    }

    #[test]
    fn parses_six_ghz_rnr_neighbor() {
        let parsed = parse(
            &[
                201, 17,
                0, 13, 134, 197, // 1 TBTT, length 13, 6 GHz 160 MHz op class, channel 197
                255, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, // TBTT offset + synthetic BSSID
                0, 0, 0, 0, 0, 0, // remaining TBTT fields
            ],
            0,
            5745,
        );

        assert_eq!(
            parsed.rnr_neighbors,
            vec![NeighborAp {
                bssid: "02:00:00:00:06:01".into(),
                channel: 197,
                frequency_mhz: 6935,
                channel_width_mhz: 160,
            }]
        );
    }
}
