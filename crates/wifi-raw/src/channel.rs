//! Shared Wi-Fi channel/frequency helpers.

pub const SIX_GHZ_BASE_MHZ: u32 = 5950;

pub fn channel_from_freq(freq_mhz: u32) -> u16 {
    match freq_mhz {
        2484 => 14,
        2401..=2495 => ((freq_mhz as i32 - 2407) / 5) as u16,
        4900..=5895 => ((freq_mhz as i32 - 5000) / 5) as u16,
        5935 => 2,
        5955..=7115 => ((freq_mhz as i32 - SIX_GHZ_BASE_MHZ as i32) / 5) as u16,
        _ => 0,
    }
}

pub fn freq_from_channel(channel: u16, band: ChannelBand) -> u32 {
    match band {
        ChannelBand::Band2_4 => {
            if channel == 14 {
                2484
            } else {
                2407 + channel as u32 * 5
            }
        }
        ChannelBand::Band5 => 5000 + channel as u32 * 5,
        ChannelBand::Band6 => {
            if channel == 2 {
                5935
            } else {
                SIX_GHZ_BASE_MHZ + channel as u32 * 5
            }
        }
        ChannelBand::Unknown => 0,
    }
}

/// DFS channels are the 5 GHz UNII-2/2e band (52-144) requiring radar avoidance.
pub fn is_dfs(freq_mhz: u32, channel: u16) -> bool {
    (5000..5900).contains(&freq_mhz) && (52..=144).contains(&channel)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelBand {
    Band2_4,
    Band5,
    Band6,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_frequencies_to_channels() {
        assert_eq!(channel_from_freq(2412), 1);
        assert_eq!(channel_from_freq(2484), 14);
        assert_eq!(channel_from_freq(5180), 36);
        assert_eq!(channel_from_freq(5935), 2);
        assert_eq!(channel_from_freq(5955), 1);
    }

    #[test]
    fn maps_channels_to_primary_frequencies() {
        assert_eq!(freq_from_channel(1, ChannelBand::Band2_4), 2412);
        assert_eq!(freq_from_channel(14, ChannelBand::Band2_4), 2484);
        assert_eq!(freq_from_channel(36, ChannelBand::Band5), 5180);
        assert_eq!(freq_from_channel(2, ChannelBand::Band6), 5935);
        assert_eq!(freq_from_channel(5, ChannelBand::Band6), 5975);
    }
}
