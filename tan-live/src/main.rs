//! Live desktop audio through TAN. Captures either your microphone or (via
//! WASAPI loopback on Windows) whatever is currently playing on an output
//! device, runs it through the real-time engine, and plays the result out a
//! second device.
//!
//! Capture and playback run on two independent audio threads. They hand
//! samples across a lock-free single-producer / single-consumer ring buffer
//! (see `Ring` below) - no mutex in either audio callback, so neither thread
//! can ever block the other and cause a dropout. The consumer caps latency
//! by discarding the oldest audio when the buffer runs long (the two device
//! clocks drift), which it can do safely because it alone owns the read
//! index.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::cell::UnsafeCell;
use std::env;
use std::process::exit;
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
/// the other's with an `Acquire` load, which is exactly the handshake needed
/// for the sample writes/reads to be visible across threads.
struct Ring {
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
    fn new(cap: usize) -> Self {
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
    /// full - a safety valve that should never trigger in practice because the
    /// consumer trims the buffer back every callback.
    fn push_slice(&self, src: &[f32]) -> usize {
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
    fn pop_slice(&self, dst: &mut [f32]) -> usize {
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
    /// oldest down to `target`. This caps end-to-end latency when the capture
    /// clock runs slightly ahead of playback: rather than let delay grow without
    /// bound, we skip forward. Safe because only the consumer moves `head`.
    fn trim_to(&self, target: usize) {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Relaxed);
        let avail = tail.wrapping_sub(head);
        if avail > target {
            self.head.store(head.wrapping_add(avail - target), Ordering::Release);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let host = cpal::default_host();

    if args.iter().any(|a| a == "--list-devices") || args.iter().any(|a| a == "--help" || a == "-h") {
        list_devices(&host);
        return;
    }

    let inputs: Vec<cpal::Device> = host
        .input_devices()
        .expect("failed to enumerate input devices")
        .collect();
    let outputs: Vec<cpal::Device> = host
        .output_devices()
        .expect("failed to enumerate output devices")
        .collect();

    let loopback_from = flag_value(&args, "--loopback-from");
    let loopback = loopback_from.is_some() || args.iter().any(|a| a == "--loopback");
    let input_spec = flag_value(&args, "--input");
    let output_spec = flag_value(&args, "--output");
    let latency_ms: u64 = flag_value(&args, "--latency-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    let profile = match flag_value(&args, "--profile").as_deref() {
        None | Some("movie") => Profile::movie(),
        Some("music") => Profile::music(),
        Some(other) => {
            eprintln!("unknown profile '{other}' (expected: movie, music)");
            exit(1);
        }
    };

    // Capture: on Windows, loopback captures an *output* device (its playing
    // audio); otherwise we capture an input (microphone).
    let capture_device = if loopback {
        resolve(&outputs, loopback_from.as_deref(), || host.default_output_device())
            .unwrap_or_else(|| fail("no output device to loop back from", &outputs))
    } else {
        resolve(&inputs, input_spec.as_deref(), || host.default_input_device())
            .unwrap_or_else(|| fail("no input (microphone) device", &inputs))
    };
    println!(
        "capture:  {} ({})",
        capture_device.to_string(),
        if loopback { "loopback" } else { "microphone" }
    );

    let playback_device = resolve(&outputs, output_spec.as_deref(), || host.default_output_device())
        .unwrap_or_else(|| fail("no output device for playback", &outputs));
    println!("playback: {}", playback_device.to_string());

    if loopback && output_spec.is_none() {
        eprintln!(
            "warning: capturing loopback from the default output while also playing back to it \
             will double up audio (original + processed). Pass --output \"<name-or-index>\" to send \
             TAN's output somewhere else (headphones, a virtual cable). Run --list-devices to see options."
        );
    }

    let capture_config = capture_device
        .default_input_config()
        .expect("capture device has no usable input config (on Windows, loopback needs the \
                 output device's format, which cpal exposes as its input config)");
    let sample_rate = capture_config.sample_rate();
    let channels = capture_config.channels() as usize;
    let rate_hz = sample_rate as u64;
    println!("format:   {} Hz, {} channel(s)", rate_hz, channels);

    // Latency budget, in samples: how much audio we allow to sit buffered
    // before the consumer skips forward. Capacity is generously larger.
    let target = ((rate_hz * latency_ms / 1000) as usize) * channels;
    let cap = (rate_hz as usize * channels).max(target * 3).max(1024);
    println!("latency:  ~{latency_ms} ms buffered (ring holds up to {} samples)", cap);

    let ring = Arc::new(Ring::new(cap));
    let mut normalizer = Normalizer::new(sample_rate, channels, profile);

    // Capture callback (producer). Copies the device buffer into a reusable
    // scratch (no per-callback allocation after warmup), runs TAN in place,
    // then pushes into the ring.
    let ring_in = ring.clone();
    let mut scratch: Vec<f32> = Vec::new();
    let input_stream = capture_device
        .build_input_stream(
            capture_config.into(),
            move |data: &[f32], _| {
                scratch.clear();
                scratch.extend_from_slice(data);
                normalizer.process(&mut scratch);
                ring_in.push_slice(&scratch);
            },
            |err| eprintln!("capture stream error: {err}"),
            None,
        )
        .expect("failed to build capture stream");

    // Playback callback (consumer). Trims stale backlog to the latency target,
    // then drains the ring into the output, zero-filling any shortfall.
    let ring_out = ring.clone();
    let playback_config = cpal::StreamConfig {
        channels: channels as u16,
        sample_rate,
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
        .expect("failed to build playback stream");

    input_stream.play().expect("failed to start capture");
    output_stream.play().expect("failed to start playback");

    println!("TAN live is running. Press Ctrl+C to stop.");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

/// Resolve a device from a `--flag` value that may be either a numeric index
/// (as shown by `--list-devices`) or a case-insensitive name substring. Falls
/// back to `default` when no value was given.
fn resolve(
    devices: &[cpal::Device],
    spec: Option<&str>,
    default: impl FnOnce() -> Option<cpal::Device>,
) -> Option<cpal::Device> {
    match spec {
        None => default(),
        Some(s) => {
            if let Ok(idx) = s.parse::<usize>() {
                if let Some(d) = devices.get(idx) {
                    return Some(d.clone());
                }
                eprintln!("device index {idx} is out of range");
                exit(1);
            }
            let needle = s.to_lowercase();
            match devices.iter().find(|d| d.to_string().to_lowercase().contains(&needle)) {
                Some(d) => Some(d.clone()),
                None => {
                    eprintln!("no device matching '{s}'");
                    exit(1);
                }
            }
        }
    }
}

fn fail(msg: &str, devices: &[cpal::Device]) -> ! {
    eprintln!("{msg}. Available:");
    for (i, d) in devices.iter().enumerate() {
        eprintln!("  [{i}] {}", d.to_string());
    }
    exit(1);
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::Ring;

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
        // Fill, drain most, then push across the modulo boundary.
        assert_eq!(r.push_slice(&[1.0, 2.0, 3.0]), 3);
        let mut two = [0.0; 2];
        assert_eq!(r.pop_slice(&mut two), 2); // consumes 1,2; head now at 2
        assert_eq!(r.push_slice(&[4.0, 5.0, 6.0]), 3); // wraps past index 3
        let mut four = [0.0; 4];
        assert_eq!(r.pop_slice(&mut four), 4);
        assert_eq!(four, [3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn full_buffer_drops_newest() {
        let r = Ring::new(4);
        assert_eq!(r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 4); // only 4 fit
        let mut out = [0.0; 4];
        assert_eq!(r.pop_slice(&mut out), 4);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]); // newest (5,6) were dropped
    }

    #[test]
    fn trim_discards_oldest_down_to_target() {
        let r = Ring::new(16);
        r.push_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        r.trim_to(2); // keep only the 2 newest
        let mut out = [0.0; 8];
        let n = r.pop_slice(&mut out);
        assert_eq!(n, 2);
        assert_eq!(&out[..2], &[5.0, 6.0]);
    }

    #[test]
    fn trim_noop_when_under_target() {
        let r = Ring::new(16);
        r.push_slice(&[1.0, 2.0]);
        r.trim_to(8); // nothing to discard
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

    // Producer and consumer on separate threads, with the producer spinning
    // until each push lands (so nothing is dropped): the consumer must see
    // exactly 0,1,2,...,N-1 in order. This is the real test of the atomic
    // handshake - a bug in the memory ordering shows up as a gap or dupe.
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
                    let v = [i as f32];
                    if ring.push_slice(&v) == 1 {
                        i += 1;
                    } else {
                        std::hint::spin_loop(); // full; let the consumer drain
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

fn list_devices(host: &cpal::Host) {
    println!("input devices (microphones):");
    match host.input_devices() {
        Ok(devs) => {
            for (i, d) in devs.enumerate() {
                println!("  [{i}] {}", d.to_string());
            }
        }
        Err(e) => println!("  (failed to enumerate: {e})"),
    }
    println!("output devices (speakers / headphones / virtual cables):");
    match host.output_devices() {
        Ok(devs) => {
            for (i, d) in devs.enumerate() {
                println!("  [{i}] {}", d.to_string());
            }
        }
        Err(e) => println!("  (failed to enumerate: {e})"),
    }
    println!(
        "\nusage: tan-live [options]\n\
         \n\
         Capture (pick one):\n\
         \x20 --loopback                 capture the default output device (what's playing)\n\
         \x20 --loopback-from <name|idx> capture a specific output device via loopback\n\
         \x20 --input <name|idx>         capture a microphone (default if no capture flag)\n\
         \n\
         Playback:\n\
         \x20 --output <name|idx>        device to play TAN's result out of\n\
         \n\
         Other:\n\
         \x20 --profile movie|music      DSP profile (default: movie)\n\
         \x20 --latency-ms <n>           buffered latency target (default: 200)\n\
         \x20 --list-devices             show this list and exit\n\
         \n\
         Names match case-insensitively as a substring; indices are the [n] shown above.\n\
         Tip: to normalize a movie without hearing it twice, send --output to headphones\n\
         or a virtual audio cable, not the same speakers you're capturing."
    );
}
