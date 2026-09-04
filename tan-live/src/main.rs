//! Live desktop audio through TAN. Captures either your microphone or (via
//! WASAPI loopback on Windows) whatever is currently playing on an output
//! device, runs it through the real-time engine, and plays the result out a
//! second device.
//!
//! This is a first cut: the capture and playback callbacks hand samples off
//! through a mutex-guarded ring buffer rather than a lock-free one. That's
//! not what a final low-latency build should ship with, but it's simple,
//! correct, and enough to prove the pipeline end to end on real hardware
//! before investing in a proper OS-level audio adapter (Windows Audio
//! Processing Object) later.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::env;
use std::process::exit;
use std::sync::{Arc, Mutex};
use tan_core::{Normalizer, Profile};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let host = cpal::default_host();

    if args.iter().any(|a| a == "--list-devices") {
        list_devices(&host);
        return;
    }

    let loopback = args.iter().any(|a| a == "--loopback");
    let output_name = flag_value(&args, "--output");
    let profile = match flag_value(&args, "--profile").as_deref() {
        None | Some("movie") => Profile::movie(),
        Some("music") => Profile::music(),
        Some(other) => {
            eprintln!("unknown profile '{other}' (expected: movie, music)");
            exit(1);
        }
    };

    let capture_device = if loopback {
        host.default_output_device()
            .expect("no default output device to loop back from")
    } else {
        host.default_input_device()
            .expect("no default input (microphone) device")
    };
    println!(
        "capture: {} ({})",
        capture_device.name().unwrap_or_else(|_| "?".into()),
        if loopback { "loopback" } else { "microphone" }
    );

    let playback_device = match &output_name {
        Some(name) => host
            .output_devices()
            .expect("failed to enumerate output devices")
            .find(|d| {
                d.name()
                    .map(|n| n.to_lowercase().contains(&name.to_lowercase()))
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| panic!("no output device matching '{name}'")),
        None => host
            .default_output_device()
            .expect("no default output device"),
    };
    println!(
        "playback: {}",
        playback_device.name().unwrap_or_else(|_| "?".into())
    );

    if loopback && output_name.is_none() {
        eprintln!(
            "warning: capturing loopback from the default output while also playing back to it \
             will double up audio (original + processed). Pass --output \"<device name>\" to send \
             TAN's output somewhere else (headphones, a virtual cable), or --list-devices to see options."
        );
    }

    let capture_config = capture_device
        .default_input_config()
        .expect("capture device has no usable input config (on Windows, loopback needs the \
                 output device's format, which cpal exposes as its input config)");
    let sample_rate = capture_config.sample_rate().0;
    let channels = capture_config.channels() as usize;
    println!("format: {sample_rate} Hz, {channels} channel(s)");

    let ring: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(sample_rate as usize)));
    let mut normalizer = Normalizer::new(sample_rate, channels, profile);

    let ring_in = ring.clone();
    let input_stream = capture_device
        .build_input_stream(
            &capture_config.into(),
            move |data: &[f32], _| {
                let mut buf = data.to_vec();
                normalizer.process(&mut buf);
                let mut ring = ring_in.lock().unwrap();
                ring.extend(buf);
                // Cap buffered latency: if the output side falls behind,
                // drop the oldest audio rather than let delay grow forever.
                let max_len = sample_rate as usize * channels; // ~1s
                while ring.len() > max_len {
                    ring.pop_front();
                }
            },
            |err| eprintln!("capture stream error: {err}"),
            None,
        )
        .expect("failed to build capture stream");

    let playback_config: cpal::StreamConfig = cpal::StreamConfig {
        channels: channels as u16,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    let ring_out = ring.clone();
    let output_stream = playback_device
        .build_output_stream(
            &playback_config,
            move |data: &mut [f32], _| {
                let mut ring = ring_out.lock().unwrap();
                for sample in data.iter_mut() {
                    *sample = ring.pop_front().unwrap_or(0.0);
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

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn list_devices(host: &cpal::Host) {
    println!("input devices:");
    for d in host.input_devices().expect("failed to enumerate input devices") {
        println!("  {}", d.name().unwrap_or_else(|_| "?".into()));
    }
    println!("output devices:");
    for d in host.output_devices().expect("failed to enumerate output devices") {
        println!("  {}", d.name().unwrap_or_else(|_| "?".into()));
    }
    println!(
        "\nusage: tan-live [--loopback] [--output \"<device name>\"] [--profile movie|music]"
    );
}
