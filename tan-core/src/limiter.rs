use crate::loudness::smoothing_coef;
use std::collections::VecDeque;

/// Look-ahead peak limiter. Audio is delayed by the look-ahead window while
/// the required gain is computed from the *incoming* (future) samples, so the
/// gain can ramp down before a peak arrives instead of reacting after it —
/// no overshoot, no click.
pub struct Limiter {
    ceiling: f32,
    delay: Vec<Vec<f32>>,
    delay_len: usize,
    pos: usize,
    /// Sliding-window minimum of required gain over the look-ahead window,
    /// kept as a monotonic deque of (frame index, required gain).
    window: VecDeque<(u64, f32)>,
    frame_index: u64,
    gain: f32,
    attack_coef: f32,
    release_coef: f32,
}

impl Limiter {
    pub fn new(sample_rate: u32, channels: usize, ceiling: f32, lookahead_s: f32) -> Self {
        let delay_len = ((lookahead_s * sample_rate as f32) as usize).max(1);
        Self {
            ceiling,
            delay: vec![vec![0.0; delay_len]; channels],
            delay_len,
            pos: 0,
            window: VecDeque::with_capacity(delay_len),
            frame_index: 0,
            gain: 1.0,
            // Attack spread over roughly half the look-ahead so the ramp
            // lands in time; slower release so gain recovers smoothly.
            attack_coef: smoothing_coef(lookahead_s * 0.5, sample_rate as f32),
            release_coef: smoothing_coef(0.060, sample_rate as f32),
        }
    }

    pub fn latency_frames(&self) -> usize {
        self.delay_len
    }

    /// Process one frame in place. Input goes into the delay line; the frame
    /// that comes out is `delay_len` frames old, scaled by a gain that already
    /// knows about every peak between it and "now".
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        let peak = frame.iter().fold(0.0f32, |acc, x| acc.max(x.abs()));
        let required = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };

        while let Some(&(_, g)) = self.window.back() {
            if g >= required {
                self.window.pop_back();
            } else {
                break;
            }
        }
        self.window.push_back((self.frame_index, required));
        while let Some(&(i, _)) = self.window.front() {
            if self.frame_index - i >= self.delay_len as u64 {
                self.window.pop_front();
            } else {
                break;
            }
        }
        let target = self.window.front().map(|&(_, g)| g).unwrap_or(1.0);

        let coef = if target < self.gain {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.gain = coef * self.gain + (1.0 - coef) * target;

        for (ch, sample) in frame.iter_mut().enumerate() {
            let delayed = self.delay[ch][self.pos];
            self.delay[ch][self.pos] = *sample;
            // Hard clamp as a final guarantee; smoothing alone can let a
            // fraction of a dB through on pathological input.
            *sample = (delayed * self.gain).clamp(-self.ceiling, self.ceiling);
        }
        self.pos = (self.pos + 1) % self.delay_len;
        self.frame_index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_never_exceeds_ceiling() {
        let mut limiter = Limiter::new(48000, 1, 0.9, 0.008);
        let mut peak = 0.0f32;
        for n in 0..48000 {
            // Loud sine with a 2x overshoot burst in the middle.
            let amp = if (20000..22000).contains(&n) { 2.0 } else { 0.95 };
            let mut frame =
                [amp * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48000.0).sin()];
            limiter.process_frame(&mut frame);
            peak = peak.max(frame[0].abs());
        }
        assert!(peak <= 0.9 + 1e-4, "peak {peak} exceeded ceiling");
    }

    #[test]
    fn quiet_audio_passes_untouched() {
        let mut limiter = Limiter::new(48000, 1, 0.9, 0.008);
        let latency = limiter.latency_frames();
        let mut input = Vec::new();
        let mut output = Vec::new();
        for n in 0..9600 {
            let x = 0.3 * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 48000.0).sin();
            input.push(x);
            let mut frame = [x];
            limiter.process_frame(&mut frame);
            output.push(frame[0]);
        }
        for i in 0..(9600 - latency) {
            let diff = (input[i] - output[i + latency]).abs();
            assert!(diff < 1e-6, "sample {i} altered by {diff}");
        }
    }
}
