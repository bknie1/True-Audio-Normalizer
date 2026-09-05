//! Live desktop audio through TAN (command line). Captures either your
//! microphone or (via WASAPI loopback on Windows) whatever is currently
//! playing on an output device, runs it through the real-time engine, and
//! plays the result out a second device. The engine itself lives in
//! `tan_live::{start, EngineConfig, ...}` and is shared with the tray app.

use cpal::traits::{DeviceTrait, HostTrait};
use std::env;
use std::process::exit;
use tan_live::{start, EngineConfig, ProfileKind};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "--list-devices" || a == "--help" || a == "-h") {
        list_devices();
        return;
    }

    let loopback_from = flag_value(&args, "--loopback-from");
    let cfg = EngineConfig {
        loopback: loopback_from.is_some() || args.iter().any(|a| a == "--loopback"),
        capture: loopback_from.or_else(|| flag_value(&args, "--input")),
        output: flag_value(&args, "--output"),
        profile: match flag_value(&args, "--profile").as_deref() {
            None | Some("movie") => ProfileKind::Movie,
            Some("music") => ProfileKind::Music,
            Some(other) => {
                eprintln!("unknown profile '{other}' (expected: movie, music)");
                exit(1);
            }
        },
        latency_ms: flag_value(&args, "--latency-ms")
            .and_then(|s| s.parse().ok())
            .unwrap_or(200),
    };

    if cfg.loopback && cfg.output.is_none() {
        eprintln!(
            "warning: capturing loopback from the default output while also playing back to it \
             will double up audio (original + processed). Pass --output \"<name-or-index>\" to send \
             TAN's output somewhere else (headphones, a virtual cable). Run --list-devices to see options."
        );
    }

    let engine = match start(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("could not start: {e}");
            exit(1);
        }
    };
    println!("{}", engine.info);
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

fn list_devices() {
    let host = cpal::default_host();
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
