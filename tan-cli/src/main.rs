mod wav;

use std::env;
use std::process::exit;
use tan_core::{LoudnessMeter, Normalizer, Profile};
use wav::WavSpec;

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("gen") if args.len() == 3 => generate(&args[2]),
        Some("process") if args.len() == 4 || args.len() == 5 => {
            let profile = match args.get(4).map(String::as_str) {
                None | Some("movie") => Profile::movie(),
                Some("music") => Profile::music(),
                Some(other) => {
                    eprintln!("unknown profile '{other}' (expected: movie, music)");
                    exit(1);
                }
            };
            process(&args[2], &args[3], profile);
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  tan-cli gen <out.wav>                        generate a badly-mixed demo file");
            eprintln!("  tan-cli process <in.wav> <out.wav> [profile] normalize a wav (profile: movie, music)");
            exit(1);
        }
    }
}

const SAMPLE_RATE: u32 = 48000;

/// Simulates a poorly mixed movie soundtrack: quiet dialogue-like passages
/// (modulated midrange tone) alternating with loud action bursts (bass hits
/// plus noise). Deterministic, so demo results are reproducible.
fn generate(path: &str) {
    let section = 3 * SAMPLE_RATE as usize;
    let mut samples = Vec::with_capacity(section * 4 * 2);
    let mut noise_state: u64 = 0x2545F4914F6CDD1D;
    let mut noise = || {
        noise_state = noise_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((noise_state >> 40) as f32 / (1 << 23) as f32) - 1.0
    };

    for block in 0..4 {
        let loud = block % 2 == 1;
        for n in 0..section {
            let t = n as f32 / SAMPLE_RATE as f32;
            let sample = if loud {
                0.55 * (2.0 * std::f32::consts::PI * 65.0 * t).sin()
                    + 0.25 * noise()
                    + 0.15 * (2.0 * std::f32::consts::PI * 900.0 * t).sin()
            } else {
                // Speech-ish: 220 Hz with a syllable-rate wobble and a formant overtone.
                let syllables = 0.6 + 0.4 * (2.0 * std::f32::consts::PI * 3.0 * t).sin();
                0.03 * syllables
                    * ((2.0 * std::f32::consts::PI * 220.0 * t).sin()
                        + 0.4 * (2.0 * std::f32::consts::PI * 880.0 * t).sin())
            };
            samples.push(sample); // left
            samples.push(sample); // right
        }
    }

    let spec = WavSpec {
        sample_rate: SAMPLE_RATE,
        channels: 2,
        bits_per_sample: 16,
    };
    wav::write_wav(path, &spec, &samples).expect("failed to write wav");
    println!("wrote {path}: 12s stereo, alternating quiet dialogue / loud action");
}

fn process(input: &str, output: &str, profile: Profile) {
    let (spec, mut samples) = wav::read_wav(input).expect("failed to read wav");
    let channels = spec.channels as usize;

    let before = Stats::measure(&samples, spec.sample_rate, channels);

    let mut normalizer = Normalizer::new(spec.sample_rate, channels, profile);
    normalizer.process(&mut samples);

    // The limiter delays audio by its look-ahead. Flush that many frames of
    // silence through, then drop the same amount from the front, so the
    // output lines up with the input.
    let latency = normalizer.latency_frames() * channels;
    let mut tail = vec![0.0f32; latency];
    normalizer.process(&mut tail);
    samples.extend_from_slice(&tail);
    samples.drain(..latency);

    let after = Stats::measure(&samples, spec.sample_rate, channels);
    wav::write_wav(output, &spec, &samples).expect("failed to write wav");

    println!("{:<22} {:>10} {:>10}", "", "before", "after");
    println!(
        "{:<22} {:>10.1} {:>10.1}",
        "quiet passages (dB)", before.quiet_db, after.quiet_db
    );
    println!(
        "{:<22} {:>10.1} {:>10.1}",
        "loud passages (dB)", before.loud_db, after.loud_db
    );
    println!(
        "{:<22} {:>10.1} {:>10.1}",
        "loudness range (dB)",
        before.loud_db - before.quiet_db,
        after.loud_db - after.quiet_db
    );
    println!(
        "{:<22} {:>10.2} {:>10.2}",
        "peak (linear)", before.peak, after.peak
    );
    println!("wrote {output}");
}

struct Stats {
    quiet_db: f32,
    loud_db: f32,
    peak: f32,
}

impl Stats {
    /// Loudness sampled every 100 ms, ignoring near-silence. Readings are
    /// split at the midpoint between the softest and loudest observed levels,
    /// and each side is averaged: "quiet" is the typical level of the soft
    /// material, "loud" the typical level of the loud material.
    fn measure(samples: &[f32], sample_rate: u32, channels: usize) -> Self {
        let mut meter = LoudnessMeter::new(sample_rate, channels);
        let stride = (sample_rate as usize / 10) * channels;
        let mut readings = Vec::new();
        for (i, frame) in samples.chunks_exact(channels).enumerate() {
            let db = meter.process_frame(frame);
            if (i * channels) % stride == 0 && db > -70.0 {
                readings.push(db);
            }
        }
        let min = readings.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = readings.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let midpoint = (min + max) / 2.0;
        let mean = |side: Vec<f32>| -> f32 {
            if side.is_empty() {
                return f32::NEG_INFINITY;
            }
            side.iter().sum::<f32>() / side.len() as f32
        };
        Self {
            quiet_db: mean(readings.iter().cloned().filter(|&r| r < midpoint).collect()),
            loud_db: mean(readings.iter().cloned().filter(|&r| r >= midpoint).collect()),
            peak: samples.iter().fold(0.0f32, |a, x| a.max(x.abs())),
        }
    }
}
