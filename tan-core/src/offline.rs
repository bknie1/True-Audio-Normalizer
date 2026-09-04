use crate::limiter::Limiter;
use crate::loudness::LoudnessMeter;
use crate::Profile;

/// Offline two-pass normalization. Unlike the real-time path, this sees the
/// whole file before deciding anything, so gain can ramp down *ahead* of a
/// loud onset and every change happens at an inaudibly gentle rate. The
/// result has none of the reactive artifacts a live processor is stuck with.
///
/// Pass 1 measures loudness on a 10 ms grid. The desired gain curve is then
/// slew-rate limited twice: forward in time (changes can't happen too fast)
/// and backward in time (so a cut propagates earlier, starting before the
/// onset that requires it). Taking the pointwise minimum of the two keeps
/// both constraints. Pass 2 applies the curve and runs the peak limiter as a
/// final safety net.
pub fn normalize_offline(samples: &mut Vec<f32>, sample_rate: u32, channels: usize, p: Profile) {
    let hop = (sample_rate as usize / 100).max(1); // 10 ms of frames
    let frames = samples.len() / channels;
    if frames == 0 {
        return;
    }

    // Pass 1: loudness per hop.
    let mut meter = LoudnessMeter::new(sample_rate, channels);
    let mut loudness = Vec::with_capacity(frames / hop + 2);
    for (i, frame) in samples.chunks_exact(channels).enumerate() {
        let db = meter.process_frame(frame);
        if i % hop == 0 {
            loudness.push(db);
        }
    }

    // Desired gain per hop; during gated (near-silent) stretches, hold the
    // previous value so silence never gets boosted.
    let mut desired = Vec::with_capacity(loudness.len());
    let mut held = 0.0f32;
    for &db in &loudness {
        if db > p.gate_db {
            held = ((p.target_db - db) * p.strength).clamp(-p.max_cut_db, p.max_boost_db);
        }
        desired.push(held);
    }

    let hop_s = hop as f32 / sample_rate as f32;
    let n = desired.len();

    // The meter takes ~50 ms to register a loud onset, so `desired` has a
    // brief "not loud yet" bump at each onset. Offline we can simply look a
    // few hops ahead and take the minimum, cancelling that reaction lag.
    let anticipation_hops = 6;
    let desired: Vec<f32> = (0..n)
        .map(|i| {
            desired[i..(i + anticipation_hops).min(n)]
                .iter()
                .cloned()
                .fold(f32::INFINITY, f32::min)
        })
        .collect();

    // Forward pass: gain may not change faster than the live rates allow.
    let mut forward = desired.clone();
    for i in 1..n {
        let prev = forward[i - 1];
        let target = desired[i];
        forward[i] = if target > prev {
            let rate = if prev < 0.0 {
                p.recover_db_per_s
            } else {
                p.rise_db_per_s
            };
            (prev + rate * hop_s).min(target)
        } else {
            (prev - p.fall_db_per_s * hop_s).max(target)
        };
    }

    // Backward pass: walking back from each cut, gain may only climb at the
    // pre-duck rate. In forward time that becomes a quick ramp down that
    // lands exactly at the onset, so the cut is fully in place when the loud
    // sound arrives without audibly ducking the preceding content.
    let mut backward = desired.clone();
    for i in (0..n - 1).rev() {
        let next = backward[i + 1];
        if desired[i] > next {
            backward[i] = (next + p.preduck_db_per_s * hop_s).min(desired[i]);
        }
    }

    // The lower of the two curves satisfies both constraints.
    let gain_db: Vec<f32> = forward
        .iter()
        .zip(&backward)
        .map(|(f, b)| f.min(*b))
        .collect();
    let gain_lin: Vec<f32> = gain_db.iter().map(|db| 10.0f32.powf(db / 20.0)).collect();

    // Pass 2: apply the curve (linear interpolation between hop points),
    // then limit peaks.
    for (i, frame) in samples.chunks_exact_mut(channels).enumerate() {
        let hop_idx = i / hop;
        let frac = (i % hop) as f32 / hop as f32;
        let a = gain_lin[hop_idx.min(gain_lin.len() - 1)];
        let b = gain_lin[(hop_idx + 1).min(gain_lin.len() - 1)];
        let gain = a + (b - a) * frac;
        for sample in frame.iter_mut() {
            *sample *= gain;
        }
    }

    let mut limiter = Limiter::new(sample_rate, channels, p.ceiling, p.lookahead_s);
    for frame in samples.chunks_exact_mut(channels) {
        limiter.process_frame(frame);
    }
    // Flush the limiter's delay and re-align so output matches input timing.
    let latency = limiter.latency_frames() * channels;
    let mut tail = vec![0.0f32; latency];
    for frame in tail.chunks_exact_mut(channels) {
        limiter.process_frame(frame);
    }
    samples.extend_from_slice(&tail);
    samples.drain(..latency);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, freq: f32, seconds: f32, sr: u32) -> Vec<f32> {
        (0..(seconds * sr as f32) as usize)
            .map(|n| amplitude * (2.0 * std::f32::consts::PI * freq * n as f32 / sr as f32).sin())
            .collect()
    }

    fn loudness_of(samples: &[f32], sr: u32) -> f32 {
        let mut meter = LoudnessMeter::new(sr, 1);
        let mut db = f32::MIN;
        for s in samples {
            db = meter.process_frame(&[*s]);
        }
        db
    }

    #[test]
    fn onset_arrives_already_controlled() {
        let sr = 48000;
        let mut buf = Vec::new();
        buf.extend(sine(0.02, 300.0, 2.0, sr));
        buf.extend(sine(0.6, 300.0, 2.0, sr));
        normalize_offline(&mut buf, sr, 1, Profile::movie());

        let onset = 2 * sr as usize;
        let window = |start_ms_after: usize| {
            let start = onset + start_ms_after * sr as usize / 1000;
            loudness_of(&buf[start..start + 150 * sr as usize / 1000], sr)
        };
        // The very first window after the onset must already sit at the
        // settled level; offline mode has no excuse for a hot start.
        let immediate = window(0);
        let settled = window(1350);
        assert!(
            (immediate - settled).abs() < 2.0,
            "offline onset should arrive pre-ducked: immediate {immediate}, settled {settled}"
        );
    }

    #[test]
    fn range_is_reduced_more_than_live() {
        let sr = 48000;
        let mut buf = Vec::new();
        buf.extend(sine(0.02, 300.0, 3.0, sr));
        buf.extend(sine(0.6, 300.0, 3.0, sr));
        let quiet_in = loudness_of(&buf[2 * sr as usize..3 * sr as usize], sr);
        let loud_in = loudness_of(&buf[5 * sr as usize..], sr);

        normalize_offline(&mut buf, sr, 1, Profile::movie());
        let quiet_out = loudness_of(&buf[2 * sr as usize..3 * sr as usize], sr);
        let loud_out = loudness_of(&buf[5 * sr as usize..], sr);

        let range_in = loud_in - quiet_in;
        let range_out = loud_out - quiet_out;
        assert!(
            range_out < range_in * 0.4,
            "offline should tighten range hard: {range_in} -> {range_out}"
        );
    }

    #[test]
    fn silence_stays_silent() {
        let mut buf = vec![0.0f32; 48000 * 2];
        normalize_offline(&mut buf, 48000, 1, Profile::movie());
        let peak = buf.iter().fold(0.0f32, |a, x| a.max(x.abs()));
        assert!(peak < 1e-6, "silence should stay silent, peak {peak}");
    }
}
