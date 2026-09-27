//! `tan-stremio pcm-filter` - the streaming TAN audio filter used inside the
//! real-time proxy's ffmpeg pipeline.
//!
//! Reads raw interleaved 32-bit-float little-endian PCM on stdin, runs it
//! through tan-core's streaming `Normalizer` in place, and writes the same
//! format to stdout. ffmpeg on either side handles decode/encode and the
//! container; this stage is pure DSP so all channels are leveled together
//! (a per-channel LADSPA plugin can't do that without shifting the image).

use std::io::{self, Read, Write};
use tan_core::{Normalizer, Profile};

use crate::profile_by_name;

pub fn run(args: &[String]) -> ! {
    let rate: u32 = flag(args, "--rate").and_then(|s| s.parse().ok()).unwrap_or(48_000);
    let channels: usize = flag(args, "--channels").and_then(|s| s.parse().ok()).unwrap_or(2);
    let profile_name = flag(args, "--profile").unwrap_or_else(|| "movie".to_string());
    let Some(profile) = profile_by_name(&profile_name) else {
        eprintln!("pcm-filter: unknown profile '{profile_name}'");
        std::process::exit(2);
    };
    if channels == 0 {
        eprintln!("pcm-filter: --channels must be >= 1");
        std::process::exit(2);
    }
    std::process::exit(filter(rate, channels, profile));
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn filter(rate: u32, channels: usize, profile: Profile) -> i32 {
    let mut norm = Normalizer::new(rate, channels, profile);
    let frame_bytes = channels * 4; // one interleaved frame of f32
    // Process in blocks; size is a whole number of frames so `process` never
    // sees a partial frame. ~4096 frames keeps latency and syscalls low.
    let block_frames = 4096usize;

    let mut stdin = io::stdin().lock();
    let mut stdout = io::BufWriter::new(io::stdout().lock());

    // Leftover bytes that didn't complete a frame across a read boundary.
    let mut carry: Vec<u8> = Vec::with_capacity(frame_bytes);
    let mut raw = vec![0u8; block_frames * frame_bytes];
    let mut floats: Vec<f32> = Vec::with_capacity(block_frames * channels);

    loop {
        let n = match stdin.read(&mut raw) {
            Ok(0) => break,
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("pcm-filter: read error: {e}");
                return 1;
            }
        };
        // Combine any carry with this read, process all complete frames, keep
        // a fresh carry of the trailing partial frame (if any).
        let mut buf = &raw[..n];
        floats.clear();
        // Fast path when there's no carry and the read is frame-aligned.
        if carry.is_empty() && n % frame_bytes == 0 {
            for chunk in buf.chunks_exact(4) {
                floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
        } else {
            carry.extend_from_slice(buf);
            let complete = carry.len() - (carry.len() % frame_bytes);
            for chunk in carry[..complete].chunks_exact(4) {
                floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
            let remainder = carry.split_off(complete);
            carry = remainder;
            buf = &[];
            let _ = buf;
        }
        if floats.is_empty() {
            continue;
        }
        norm.process(&mut floats);
        // Write back as f32le.
        let mut out = Vec::with_capacity(floats.len() * 4);
        for &s in &floats {
            out.extend_from_slice(&s.to_le_bytes());
        }
        if let Err(e) = stdout.write_all(&out) {
            // Downstream ffmpeg closing the pipe (client disconnect) is normal.
            if e.kind() == io::ErrorKind::BrokenPipe {
                return 0;
            }
            eprintln!("pcm-filter: write error: {e}");
            return 1;
        }
    }
    if let Err(e) = stdout.flush() {
        if e.kind() != io::ErrorKind::BrokenPipe {
            eprintln!("pcm-filter: flush error: {e}");
            return 1;
        }
    }
    0
}
