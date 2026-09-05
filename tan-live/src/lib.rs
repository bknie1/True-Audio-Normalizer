//! The live audio engine shared by the `tan-live` CLI and the `tan-tray`
//! desktop app: device enumeration, a lock-free ring buffer, and a
//! start/stop handle that runs TAN between a capture and a playback device.
//!
//! Capture and playback run on two independent audio threads that hand samples
//! across a lock-free single-producer / single-consumer ring (`Ring`) - no
//! mutex in either audio callback, so neither can block the other and cause a
//! dropout. The consumer caps latency by discarding the oldest audio when the
//! two device clocks drift, which it can do safely because it alone owns the
//! read index. This is cross-platform via cpal (WASAPI/CoreAudio/ALSA); on
//! Windows, "loopback" capture reads an output device's own playing audio.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tan_core::{Normalizer, Profile};

/// Lock-free single-producer, single-consumer ring buffer of `f32` samples.
///
/// Indices are free-running `usize` counters (only their difference and their
/// value modulo capacity matter), so there is no empty/full ambiguity: the
/// number of buffered samples is always `tail - head`. The producer owns
/// `tail` and only ever advances it; the consumer owns `head` and only ever
/// advances it. Each side publishes its index with a `Release` store and reads
/// the other's with an `Acquire` load, the handshake that makes the sample
/// writes/reads visible across threads.
pub struct Ring {
    buf: Box<[UnsafeCell<f32>]>,
    cap: usize,
    head: AtomicUsize, // read cursor, owned by the consumer (playback)
    tail: AtomicUsize, // write cursor, owned by the producer (capture)
}

// Safe because access is disciplined SPSC: exactly one thread writes (advancing
// `tail`), exactly one reads (advancing `head`), and each slot is written
// before its `tail` publication and read only after that publication is seen.
unsafe impl Sync for Ring {}

impl Ring {
    pub fn new(cap: usize) -> Self {
        let mut v = Vec::with_capacity(cap);
        v.resize_with(cap, || UnsafeCell::new(0.0));
        Ring {
            buf: v.into_boxed_slice(),
            cap,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Producer only. Writes as much of `src` as fits, returns how many samples
    /// were written. Drops the tail of `src` (newest audio) if the buffer is
    /// full - a safety valve that should never trigger because the consumer
    /// trims the buffer back every callback.
    pub fn push_slice(&self, src: &[f32]) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        let free = self.cap - tail.wrapping_sub(head);
        let n = src.len().min(free);
        for (i, &s) in src.iter().take(n).enumerate() {
            // SAFETY: slot is within capacity and not being read (index >= tail).
            unsafe { *self.buf[tail.wrapping_add(i) % self.cap].get() = s; }
        }
        self.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer only. Fills as much of `dst` as is available, returns how many
    /// samples were written (the rest of `dst` is left for the caller to zero).
    pub fn pop_slice(&self, dst: &mut [f32]) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Relaxed);
        let avail = tail.wrapping_sub(head);
        let n = dst.len().min(avail);
        for (i, d) in dst.iter_mut().take(n).enumerate() {
            // SAFETY: slot was published by the producer (index < tail).
            *d = unsafe { *self.buf[head.wrapping_add(i) % self.cap].get() };
        }
        self.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer only. If more than `target` samples are buffered, discard the
    /// oldest down to `target`, capping end-to-end latency when the capture
    /// clock runs ahead of playback. Safe because only the consumer moves
    /// `head`.
    pub fn trim_to(&self, target: usize) {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Relaxed);
        let avail = tail.wrapping_sub(head);
        if avail > target {
            self.head.store(head.wrapping_add(avail - target), Ordering::Release);
        }
    }
}

/// Which DSP preset to run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProfileKind {
    Universal,
    Movie,
    Music,
    Speech,
    Night,
    Game,
}

impl ProfileKind {
    /// All presets, in menu order (Universal first: the catch-all default).
    pub fn all() -> [ProfileKind; 6] {
        [
            ProfileKind::Universal,
            ProfileKind::Movie,
            ProfileKind::Music,
            ProfileKind::Speech,
            ProfileKind::Night,
            ProfileKind::Game,
        ]
    }

    fn profile(self) -> Profile {
        match self {
            ProfileKind::Universal => Profile::universal(),
            ProfileKind::Movie => Profile::movie(),
            ProfileKind::Music => Profile::music(),
            ProfileKind::Speech => Profile::speech(),
            ProfileKind::Night => Profile::night(),
            ProfileKind::Game => Profile::game(),
        }
    }

    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            ProfileKind::Universal => "Universal",
            ProfileKind::Movie => "Movie",
            ProfileKind::Music => "Music",
            ProfileKind::Speech => "Speech / Podcast",
            ProfileKind::Night => "Night",
            ProfileKind::Game => "Game",
        }
    }

    /// Stable lowercase key for config files, CLI flags, and menu ids.
    pub fn key(self) -> &'static str {
        match self {
            ProfileKind::Universal => "universal",
            ProfileKind::Movie => "movie",
            ProfileKind::Music => "music",
            ProfileKind::Speech => "speech",
            ProfileKind::Night => "night",
            ProfileKind::Game => "game",
        }
    }

    pub fn from_key(s: &str) -> Option<ProfileKind> {
        ProfileKind::all().into_iter().find(|p| p.key() == s)
    }
}

/// How to wire up a live session. Device fields accept a numeric index (as
/// returned by [`list_inputs`]/[`list_outputs`]) or a case-insensitive name
/// substring; `None` means the platform default.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Capture an output device's own audio (WASAPI loopback on Windows)
    /// rather than a microphone.
    pub loopback: bool,
    /// Capture device selector. When `loopback`, this picks among output
    /// devices; otherwise among inputs. `None` = default.
    pub capture: Option<String>,
    /// Playback device selector (`None` = default output).
    pub output: Option<String>,
    pub profile: ProfileKind,
    /// Buffered-latency target in milliseconds.
    pub latency_ms: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            loopback: true,
            capture: None,
            output: None,
            profile: ProfileKind::Universal,
            latency_ms: 200,
        }
    }
}

/// A running session. Dropping it stops both audio streams (and thus TAN).
pub struct RunningEngine {
    _input: cpal::Stream,
    _output: cpal::Stream,
    /// Human-readable summary of the wiring, for logs and tooltips.
    pub info: String,
    pub capture_name: String,
    pub output_name: String,
}

pub fn list_inputs() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|it| it.map(|d| d.to_string()).collect())
        .unwrap_or_default()
}

pub fn list_outputs() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|it| it.map(|d| d.to_string()).collect())
        .unwrap_or_default()
}

/// Converts interleaved audio from the capture format (in_ch, in_rate) to the
/// playback format (out_ch, out_rate): a per-frame channel downmix/upmix
/// followed by stateful linear-interpolation resampling. Linear resampling is
/// simple and dependency-free; it's more than adequate for dialogue and film,
/// and the common case here (96 kHz -> 48 kHz) is a clean 2:1 decimation.
struct FormatConverter {
    in_ch: usize,
    out_ch: usize,
    step: f32,       // input frames advanced per output frame (in_rate / out_rate)
    frac: f32,       // fractional position between prev and cur input frame
    prev: Vec<f32>,  // previous input frame, already downmixed to out_ch
    have_prev: bool,
    identity: bool,
    dm_scale: f32,   // headroom when summing many channels into fewer
}

impl FormatConverter {
    fn new(in_ch: usize, in_rate: u32, out_ch: usize, out_rate: u32) -> Self {
        FormatConverter {
            in_ch,
            out_ch,
            step: in_rate as f32 / out_rate as f32,
            frac: 0.0,
            prev: vec![0.0; out_ch],
            have_prev: false,
            identity: in_ch == out_ch && in_rate == out_rate,
            dm_scale: if in_ch > out_ch { 0.71 } else { 1.0 }, // ~ -3 dB
        }
    }

    /// Mix one input frame down/up into `out` (length `out_ch`).
    fn remix(&self, f: &[f32], out: &mut [f32]) {
        if self.in_ch == self.out_ch {
            out.copy_from_slice(f);
        } else if self.in_ch == 1 {
            for c in out.iter_mut() {
                *c = f[0];
            }
        } else if self.out_ch == 2 {
            let (mut l, mut r);
            match self.in_ch {
                6 => {
                    // 5.1: FL FR FC LFE BL BR
                    l = f[0] + 0.707 * f[2] + 0.707 * f[4];
                    r = f[1] + 0.707 * f[2] + 0.707 * f[5];
                }
                8 => {
                    // 7.1: FL FR FC LFE BL BR SL SR
                    l = f[0] + 0.707 * f[2] + 0.707 * f[4] + 0.707 * f[6];
                    r = f[1] + 0.707 * f[2] + 0.707 * f[5] + 0.707 * f[7];
                }
                _ => {
                    // Unknown layout: even indices left, odd indices right.
                    l = 0.0;
                    r = 0.0;
                    let (mut nl, mut nr) = (0.0f32, 0.0f32);
                    for (k, &s) in f.iter().enumerate() {
                        if k % 2 == 0 {
                            l += s;
                            nl += 1.0;
                        } else {
                            r += s;
                            nr += 1.0;
                        }
                    }
                    if nl > 0.0 {
                        l /= nl;
                    }
                    if nr > 0.0 {
                        r /= nr;
                    }
                }
            }
            out[0] = l * self.dm_scale;
            out[1] = r * self.dm_scale;
        } else if self.in_ch == 2 {
            // Stereo up to more channels: L to even, R to odd.
            for (c, o) in out.iter_mut().enumerate() {
                *o = if c % 2 == 0 { f[0] } else { f[1] };
            }
        } else {
            // Generic fallback: collapse to mono, spread across outputs.
            let m: f32 = f.iter().sum::<f32>() / self.in_ch as f32;
            for c in out.iter_mut() {
                *c = m;
            }
        }
    }

    /// Append converted output-format samples for `input` (interleaved in_ch).
    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.identity {
            out.extend_from_slice(input);
            return;
        }
        let frames = input.len() / self.in_ch;
        let mut cur = vec![0.0f32; self.out_ch];
        for f in 0..frames {
            let frame = &input[f * self.in_ch..(f + 1) * self.in_ch];
            self.remix(frame, &mut cur);
            if !self.have_prev {
                self.prev.copy_from_slice(&cur);
                self.have_prev = true;
                continue;
            }
            // Emit output frames sitting between prev and cur (linear interp).
            while self.frac < 1.0 {
                for c in 0..self.out_ch {
                    out.push(self.prev[c] + (cur[c] - self.prev[c]) * self.frac);
                }
                self.frac += self.step;
            }
            self.frac -= 1.0;
            self.prev.copy_from_slice(&cur);
        }
    }
}

/// A human-readable diagnostics dump: host, defaults, and every input/output
/// device with the config it actually offers (OK or the exact error). The
/// output-device probe uses `default_output_config`, which is the format TAN
/// captures in loopback - so an "ERR" there means that device can't be a
/// loopback source. Handy to copy to the clipboard when something won't start.
pub fn diagnostics() -> String {
    use std::fmt::Write;
    let host = cpal::default_host();
    let mut s = String::new();
    let _ = writeln!(s, "TAN diagnostics");
    let _ = writeln!(s, "version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(s, "os: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    let _ = writeln!(s, "cpal host: {:?}", host.id());

    let def_in = host.default_input_device().map(|d| d.to_string());
    let def_out = host.default_output_device().map(|d| d.to_string());
    let _ = writeln!(s, "default input:  {}", def_in.clone().unwrap_or_else(|| "(none)".into()));
    let _ = writeln!(s, "default output: {}", def_out.clone().unwrap_or_else(|| "(none)".into()));

    let _ = writeln!(s, "\ninput devices (microphones), capture format:");
    match host.input_devices() {
        Ok(devs) => {
            for (i, d) in devs.enumerate() {
                let name = d.to_string();
                let cfg = match d.default_input_config() {
                    Ok(c) => format!("OK {} Hz, {} ch, {:?}", c.sample_rate(), c.channels(), c.sample_format()),
                    Err(e) => format!("ERR {e}"),
                };
                let def = if Some(&name) == def_in.as_ref() { "  [default]" } else { "" };
                let _ = writeln!(s, "  [{i}] {name}{def}\n        {cfg}");
            }
        }
        Err(e) => { let _ = writeln!(s, "  (enumeration failed: {e})"); }
    }

    let _ = writeln!(s, "\noutput devices, render/loopback format (what TAN captures in loopback):");
    match host.output_devices() {
        Ok(devs) => {
            for (i, d) in devs.enumerate() {
                let name = d.to_string();
                let cfg = match d.default_output_config() {
                    Ok(c) => format!("OK {} Hz, {} ch, {:?}", c.sample_rate(), c.channels(), c.sample_format()),
                    Err(e) => format!("ERR {e}"),
                };
                let def = if Some(&name) == def_out.as_ref() { "  [default]" } else { "" };
                let _ = writeln!(s, "  [{i}] {name}{def}\n        {cfg}");
            }
        }
        Err(e) => { let _ = writeln!(s, "  (enumeration failed: {e})"); }
    }
    s
}

fn resolve(
    devices: &[cpal::Device],
    spec: Option<&str>,
    default: Option<cpal::Device>,
) -> Result<cpal::Device, String> {
    match spec {
        None => default.ok_or_else(|| "no default device available".to_string()),
        Some(s) => {
            if let Ok(idx) = s.parse::<usize>() {
                return devices
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| format!("device index {idx} is out of range"));
            }
            let needle = s.to_lowercase();
            devices
                .iter()
                .find(|d| d.to_string().to_lowercase().contains(&needle))
                .cloned()
                .ok_or_else(|| format!("no device matching '{s}'"))
        }
    }
}

/// Build and start a live session per `cfg`. On success TAN is already running;
/// keep the returned handle alive for as long as you want it to run.
pub fn start(cfg: &EngineConfig) -> Result<RunningEngine, String> {
    let host = cpal::default_host();
    let inputs: Vec<cpal::Device> = host.input_devices().map(|i| i.collect()).unwrap_or_default();
    let outputs: Vec<cpal::Device> = host.output_devices().map(|i| i.collect()).unwrap_or_default();

    let capture_device = if cfg.loopback {
        resolve(&outputs, cfg.capture.as_deref(), host.default_output_device())
            .map_err(|e| format!("capture (loopback): {e}"))?
    } else {
        resolve(&inputs, cfg.capture.as_deref(), host.default_input_device())
            .map_err(|e| format!("capture: {e}"))?
    };
    let playback_device = resolve(&outputs, cfg.output.as_deref(), host.default_output_device())
        .map_err(|e| format!("playback: {e}"))?;

    let capture_name = capture_device.to_string();
    let output_name = playback_device.to_string();

    // For loopback the capture device is a *render* (output) endpoint. cpal
    // turns an input stream on a render device into a loopback capture, but the
    // usable format is that device's render format - so ask for its OUTPUT
    // config here. `default_input_config()` correctly errors on a render
    // device ("Device does not support input"), which was the bug. A real
    // microphone (non-loopback) is a capture device and uses its input config.
    let capture_config = if cfg.loopback {
        capture_device
            .default_output_config()
            .map_err(|e| format!("capture device (loopback) has no usable format: {e}"))?
    } else {
        capture_device
            .default_input_config()
            .map_err(|e| format!("capture device has no usable input config: {e}"))?
    };
    let in_rate = capture_config.sample_rate();
    let in_ch = capture_config.channels() as usize;

    // The playback device runs at its own native format; TAN converts the
    // processed audio to it, so any input can feed any output (a 96 kHz 7.1
    // loopback into 48 kHz stereo headphones, say). No device pairing rules.
    let out_config = playback_device
        .default_output_config()
        .map_err(|e| format!("playback device has no usable format: {e}"))?;
    let out_rate = out_config.sample_rate();
    let out_ch = out_config.channels() as usize;

    // Ring holds output-format samples; size latency in those.
    let target = ((out_rate as u64 * cfg.latency_ms / 1000) as usize) * out_ch;
    let cap = (out_rate as usize * out_ch).max(target * 3).max(1024);

    let ring = Arc::new(Ring::new(cap));
    let mut normalizer = Normalizer::new(in_rate, in_ch, cfg.profile.profile());
    let mut converter = FormatConverter::new(in_ch, in_rate, out_ch, out_rate);

    let ring_in = ring.clone();
    let mut scratch: Vec<f32> = Vec::new();
    let mut converted: Vec<f32> = Vec::new();
    let input_stream = capture_device
        .build_input_stream(
            capture_config.into(),
            move |data: &[f32], _| {
                scratch.clear();
                scratch.extend_from_slice(data);
                normalizer.process(&mut scratch); // at the input format
                converted.clear();
                converter.process(&scratch, &mut converted); // -> output format
                ring_in.push_slice(&converted);
            },
            |err| eprintln!("capture stream error: {err}"),
            None,
        )
        .map_err(|e| format!("failed to build capture stream: {e}"))?;

    let ring_out = ring.clone();
    let playback_config = cpal::StreamConfig {
        channels: out_ch as u16,
        sample_rate: out_rate,
        buffer_size: cpal::BufferSize::Default,
    };
    let output_stream = playback_device
        .build_output_stream(
            playback_config,
            move |data: &mut [f32], _| {
                ring_out.trim_to(target);
                let n = ring_out.pop_slice(data);
                for s in &mut data[n..] {
                    *s = 0.0;
                }
            },
            |err| eprintln!("playback stream error: {err}"),
            None,
        )
        .map_err(|e| format!("failed to build playback stream: {e}"))?;

    input_stream.play().map_err(|e| format!("failed to start capture: {e}"))?;
    output_stream.play().map_err(|e| format!("failed to start playback: {e}"))?;

    let fmt = if in_rate == out_rate && in_ch == out_ch {
        format!("{in_rate} Hz, {in_ch} ch")
    } else {
        format!("{in_rate} Hz {in_ch} ch -> {out_rate} Hz {out_ch} ch")
    };
    let info = format!(
        "{} ({}) -> TAN [{}] -> {}  |  {}, ~{} ms",
        capture_name,
        if cfg.loopback { "loopback" } else { "microphone" },
        cfg.profile.label(),
        output_name,
        fmt,
        cfg.latency_ms,
    );

    Ok(RunningEngine {
        _input: input_stream,
        _output: output_stream,
        info,
        capture_name,
        output_name,
    })
}

#[cfg(test)]
mod tests {
    use super::{FormatConverter, Ring};

    #[test]
    fn converter_identity_passthrough() {
        let mut c = FormatConverter::new(2, 48000, 2, 48000);
        let mut out = Vec::new();
        c.process(&[0.1, 0.2, 0.3, 0.4], &mut out);
        assert_eq!(out, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn converter_downsamples_2to1_ratio() {
        // 96k stereo -> 48k stereo: roughly half as many frames out.
        let mut c = FormatConverter::new(2, 96000, 2, 48000);
        let frames_in = 1000;
        let mut input = Vec::with_capacity(frames_in * 2);
        for i in 0..frames_in {
            let v = (i as f32 * 0.01).sin();
            input.push(v);
            input.push(v);
        }
        let mut out = Vec::new();
        c.process(&input, &mut out);
        let frames_out = out.len() / 2;
        // ~500 out for 1000 in; allow small slack for edge/startup framing.
        assert!(
            (frames_out as i32 - 500).abs() <= 3,
            "expected ~500 output frames, got {frames_out}"
        );
    }

    #[test]
    fn converter_upsamples_1to2_ratio() {
        let mut c = FormatConverter::new(2, 48000, 2, 96000);
        let frames_in = 1000;
        let input: Vec<f32> = (0..frames_in * 2).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut out = Vec::new();
        c.process(&input, &mut out);
        let frames_out = out.len() / 2;
        assert!(
            (frames_out as i32 - 2000).abs() <= 4,
            "expected ~2000 output frames, got {frames_out}"
        );
    }

    #[test]
    fn converter_downmixes_71_to_stereo() {
        // Same rate, 8ch -> 2ch: one frame in, one frame out, left/right split.
        let mut c = FormatConverter::new(8, 48000, 2, 48000);
        // FL FR FC LFE BL BR SL SR - put 1.0 only on FL, expect L>0, R==0.
        let mut out = Vec::new();
        // need two frames because the resampler holds the first as `prev`.
        let frame = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        c.process(&frame, &mut out);
        c.process(&frame, &mut out);
        assert!(out.len() >= 2);
        let l = out[out.len() - 2];
        let r = out[out.len() - 1];
        assert!(l > 0.0, "left should carry FL, got {l}");
        assert_eq!(r, 0.0, "right should be silent for FL-only, got {r}");
    }

    #[test]
    fn roundtrip_preserves_order() {
        let r = Ring::new(8);
        assert_eq!(r.push_slice(&[1.0, 2.0, 3.0]), 3);
        let mut out = [0.0; 3];
        assert_eq!(r.pop_slice(&mut out), 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn wraps_around_capacity() {
        let r = Ring::new(4);
        assert_eq!(r.push_slice(&[1.0, 2.0, 3.0]), 3);
        let mut two = [0.0; 2];
        assert_eq!(r.pop_slice(&mut two), 2);
        assert_eq!(r.push_slice(&[4.0, 5.0, 6.0]), 3);
        let mut four = [0.0; 4];
        assert_eq!(r.pop_slice(&mut four), 4);
        assert_eq!(four, [3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn full_buffer_drops_newest() {
        let r = Ring::new(4);
        assert_eq!(r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 4);
        let mut out = [0.0; 4];
        assert_eq!(r.pop_slice(&mut out), 4);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn trim_discards_oldest_down_to_target() {
        let r = Ring::new(16);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        r.trim_to(2);
        let mut out = [0.0; 8];
        let n = r.pop_slice(&mut out);
        assert_eq!(n, 2);
        assert_eq!(&out[..2], &[5.0, 6.0]);
    }

    #[test]
    fn trim_noop_when_under_target() {
        let r = Ring::new(16);
        r.push_slice(&[1.0, 2.0]);
        r.trim_to(8);
        let mut out = [0.0; 4];
        assert_eq!(r.pop_slice(&mut out), 2);
        assert_eq!(&out[..2], &[1.0, 2.0]);
    }

    #[test]
    fn pop_from_empty_yields_nothing() {
        let r = Ring::new(4);
        let mut out = [0.0; 4];
        assert_eq!(r.pop_slice(&mut out), 0);
    }

    #[test]
    fn concurrent_producer_consumer_lose_nothing() {
        use std::sync::Arc;
        use std::thread;

        const N: usize = 200_000;
        let ring = Arc::new(Ring::new(1024));

        let prod = {
            let ring = ring.clone();
            thread::spawn(move || {
                let mut i = 0usize;
                while i < N {
                    if ring.push_slice(&[i as f32]) == 1 {
                        i += 1;
                    } else {
                        std::hint::spin_loop();
                    }
                }
            })
        };

        let mut next = 0usize;
        let mut scratch = [0.0f32; 256];
        while next < N {
            let got = ring.pop_slice(&mut scratch);
            for &v in &scratch[..got] {
                assert_eq!(v, next as f32, "out-of-order or lost sample");
                next += 1;
            }
            if got == 0 {
                std::hint::spin_loop();
            }
        }
        prod.join().unwrap();
        assert_eq!(next, N);
    }
}
