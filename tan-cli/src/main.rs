mod wav;

use std::env;
use std::process::exit;
use tan_core::{LoudnessMeter, Normalizer, Profile};
use wav::WavSpec;

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("gen") if args.len() == 3 => generate(&args[2]),
        Some("process") if args.len() >= 4 => {
            let rest: Vec<&str> = args[4..].iter().map(String::as_str).collect();
            let two_pass = rest.contains(&"--two-pass");
            let profile = match rest.iter().find(|a| !a.starts_with("--")) {
                None | Some(&"movie") => Profile::movie(),
                Some(&"music") => Profile::music(),
                Some(other) => {
                    eprintln!("unknown profile '{other}' (expected: movie, music)");
                    exit(1);
                }
            };
            process(&args[2], &args[3], profile, two_pass);
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  tan-cli gen <out.wav>");
            eprintln!("      generate a badly-mixed demo file");
            eprintln!("  tan-cli process <in.wav> <out.wav> [profile] [--two-pass]");
            eprintln!("      normalize a wav (profile: movie, music); --two-pass analyzes the whole");
            eprintln!("      file first for artifact-free gain that ramps down ahead of loud onsets");
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

fn process(input: &str, output: &str, profile: Profile, two_pass: bool) {
    let (spec, mut samples) = wav::read_wav(input).expect("failed to read wav");
    let channels = spec.channels as usize;

    let before_readings = loudness_track(&samples, spec.sample_rate, channels);
    let before_peak = peak_of(&samples);

    if two_pass {
        tan_core::normalize_offline(&mut samples, spec.sample_rate, channels, profile);
    } else {
        let mut normalizer = Normalizer::new(spec.sample_rate, channels, profile);
        normalizer.process(&mut samples);

        // The limiter delays audio by its look-ahead. Flush that many frames
        // of silence through, then drop the same amount from the front, so
        // the output lines up with the input.
        let latency = normalizer.latency_frames() * channels;
        let mut tail = vec![0.0f32; latency];
        normalizer.process(&mut tail);
        samples.extend_from_slice(&tail);
        samples.drain(..latency);
    }

    let after_readings = loudness_track(&samples, spec.sample_rate, channels);
    let (before, after) = Stats::compare(&before_readings, &after_readings);
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
        "peak (linear)",
        before_peak,
        peak_of(&samples)
    );
    println!("wrote {output}");
}

fn peak_of(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |a, x| a.max(x.abs()))
}

/// Loudness sampled every 100 ms across the file.
fn loudness_track(samples: &[f32], sample_rate: u32, channels: usize) -> Vec<f32> {
    let mut meter = LoudnessMeter::new(sample_rate, channels);
    let hop = sample_rate as usize / 10;
    let mut readings = Vec::new();
    for (i, frame) in samples.chunks_exact(channels).enumerate() {
        let db = meter.process_frame(frame);
        if i % hop == 0 {
            readings.push(db);
        }
    }
    readings
}

struct Stats {
    quiet_db: f32,
    loud_db: f32,
}

impl Stats {
    /// The quiet/loud split is decided once, from the *input*: readings are
    /// classified against the midpoint between the input's softest and
    /// loudest levels (ignoring near-silence). Both files are then averaged
    /// over those same time regions, so "quiet passages" always means "the
    /// passages that were quiet in the original" - even if the output has
    /// leveled them so well the two groups now overlap.
    fn compare(before: &[f32], after: &[f32]) -> (Self, Self) {
        let audible: Vec<usize> = (0..before.len().min(after.len()))
            .filter(|&i| before[i] > -70.0)
            .collect();
        let min = audible.iter().map(|&i| before[i]).fold(f32::INFINITY, f32::min);
        let max = audible
            .iter()
            .map(|&i| before[i])
            .fold(f32::NEG_INFINITY, f32::max);
        let midpoint = (min + max) / 2.0;

        let mean_over = |track: &[f32], indices: &[usize]| -> f32 {
            if indices.is_empty() {
                return f32::NEG_INFINITY;
            }
            indices.iter().map(|&i| track[i]).sum::<f32>() / indices.len() as f32
        };
        let quiet_idx: Vec<usize> = audible
            .iter()
            .cloned()
            .filter(|&i| before[i] < midpoint)
            .collect();
        let loud_idx: Vec<usize> = audible
            .iter()
            .cloned()
            .filter(|&i| before[i] >= midpoint)
            .collect();

        (
            Self {
                quiet_db: mean_over(before, &quiet_idx),
                loud_db: mean_over(before, &loud_idx),
            },
            Self {
                quiet_db: mean_over(after, &quiet_idx),
                loud_db: mean_over(after, &loud_idx),
            },
        )
    }
}
