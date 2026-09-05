//! Platform audio abstraction: the single seam between TAN's portable DSP and
//! each OS's native audio system. Every backend exposes the same capabilities,
//! so no operating system is a second-class citizen - the DSP above never
//! knows or cares which one it is talking to.
//!
//! Today there is one backend, [`CpalBackend`], which works everywhere via
//! cpal (WASAPI / CoreAudio / ALSA). Native backends (a direct WASAPI one that
//! reads the real channel mask, a CoreAudio one, a PipeWire one) can implement
//! the same [`AudioBackend`] trait later without touching the engine. Stream
//! setup lives in `lib.rs::start` for now; it becomes this trait's `run` next.

use cpal::traits::{DeviceTrait, HostTrait};

/// A single speaker position, including height (the `Top*` variants) so Atmos
/// bed layouts are representable. This is what position-aware leveling and
/// HRTF virtualization will key off of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelPosition {
    FrontLeft,
    FrontRight,
    FrontCenter,
    LowFrequency,
    BackLeft,
    BackRight,
    SideLeft,
    SideRight,
    TopFrontLeft,
    TopFrontRight,
    TopBackLeft,
    TopBackRight,
    Unknown,
}

impl ChannelPosition {
    /// Short label for diagnostics (FL, FR, FC, LFE, ...).
    pub fn label(self) -> &'static str {
        use ChannelPosition::*;
        match self {
            FrontLeft => "FL",
            FrontRight => "FR",
            FrontCenter => "FC",
            LowFrequency => "LFE",
            BackLeft => "BL",
            BackRight => "BR",
            SideLeft => "SL",
            SideRight => "SR",
            TopFrontLeft => "TFL",
            TopFrontRight => "TFR",
            TopBackLeft => "TBL",
            TopBackRight => "TBR",
            Unknown => "?",
        }
    }
}

/// The ordered speaker position of each interleaved channel.
#[derive(Clone, Debug)]
pub struct ChannelLayout(pub Vec<ChannelPosition>);

impl ChannelLayout {
    pub fn labels(&self) -> String {
        self.0.iter().map(|p| p.label()).collect::<Vec<_>>().join(" ")
    }
}

/// Best-guess standard layout for a channel count. cpal does not expose the
/// real channel mask, so this is an assumption a native backend will later
/// replace with the actual OS layout (WASAPI dwChannelMask, CoreAudio
/// AudioChannelLayout, PipeWire channel map).
pub fn standard_layout(channels: usize) -> ChannelLayout {
    use ChannelPosition::*;
    let v = match channels {
        1 => vec![FrontCenter],
        2 => vec![FrontLeft, FrontRight],
        3 => vec![FrontLeft, FrontRight, FrontCenter],
        4 => vec![FrontLeft, FrontRight, BackLeft, BackRight],
        6 => vec![FrontLeft, FrontRight, FrontCenter, LowFrequency, BackLeft, BackRight],
        8 => vec![
            FrontLeft, FrontRight, FrontCenter, LowFrequency, BackLeft, BackRight, SideLeft,
            SideRight,
        ],
        10 => vec![
            // 5.1.4
            FrontLeft, FrontRight, FrontCenter, LowFrequency, BackLeft, BackRight, TopFrontLeft,
            TopFrontRight, TopBackLeft, TopBackRight,
        ],
        12 => vec![
            // 7.1.4
            FrontLeft, FrontRight, FrontCenter, LowFrequency, BackLeft, BackRight, SideLeft,
            SideRight, TopFrontLeft, TopFrontRight, TopBackLeft, TopBackRight,
        ],
        n => vec![Unknown; n],
    };
    ChannelLayout(v)
}

/// What a backend can tell us about a device.
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub channels: usize,
    pub sample_rate: u32,
    pub is_default: bool,
    pub layout: ChannelLayout,
}

/// The capabilities every platform backend provides identically. Enumeration
/// and layout today; stream capture/playback move here next (per-OS), so the
/// engine depends only on this trait, never on a specific audio API.
pub trait AudioBackend {
    /// Backend name, for diagnostics (e.g. "cpal/WASAPI").
    fn name(&self) -> String;
    fn inputs(&self) -> Vec<DeviceInfo>;
    fn outputs(&self) -> Vec<DeviceInfo>;
}

/// The default cross-platform backend (cpal over WASAPI/CoreAudio/ALSA).
pub struct CpalBackend {
    host: cpal::Host,
}

impl CpalBackend {
    pub fn new() -> Self {
        CpalBackend { host: cpal::default_host() }
    }

    fn collect(
        devices: Option<Vec<cpal::Device>>,
        default_name: Option<String>,
        is_input: bool,
    ) -> Vec<DeviceInfo> {
        devices
            .unwrap_or_default()
            .into_iter()
            .map(|d| {
                let name = d.to_string();
                let cfg = if is_input {
                    d.default_input_config().ok()
                } else {
                    d.default_output_config().ok()
                };
                let (channels, sample_rate) = cfg
                    .map(|c| (c.channels() as usize, c.sample_rate()))
                    .unwrap_or((0, 0));
                DeviceInfo {
                    is_default: Some(&name) == default_name.as_ref(),
                    layout: standard_layout(channels),
                    name,
                    channels,
                    sample_rate,
                }
            })
            .collect()
    }
}

impl Default for CpalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for CpalBackend {
    fn name(&self) -> String {
        format!("cpal/{:?}", self.host.id())
    }

    fn inputs(&self) -> Vec<DeviceInfo> {
        let default = self.host.default_input_device().map(|d| d.to_string());
        let devs = self.host.input_devices().ok().map(|it| it.collect());
        Self::collect(devs, default, true)
    }

    fn outputs(&self) -> Vec<DeviceInfo> {
        let default = self.host.default_output_device().map(|d| d.to_string());
        let devs = self.host.output_devices().ok().map(|it| it.collect());
        Self::collect(devs, default, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_layouts_have_expected_shapes() {
        assert_eq!(standard_layout(2).labels(), "FL FR");
        assert_eq!(standard_layout(6).labels(), "FL FR FC LFE BL BR");
        assert_eq!(standard_layout(8).labels(), "FL FR FC LFE BL BR SL SR");
        // 7.1.4 must carry four height channels.
        let l = standard_layout(12);
        assert_eq!(l.0.len(), 12);
        assert!(l.labels().contains("TFL") && l.labels().contains("TBR"));
    }

    #[test]
    fn unknown_count_is_all_unknown_not_a_panic() {
        let l = standard_layout(23);
        assert_eq!(l.0.len(), 23);
        assert!(l.0.iter().all(|p| *p == ChannelPosition::Unknown));
    }
}
