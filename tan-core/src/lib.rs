mod biquad;
mod limiter;
mod loudness;

pub use limiter::Limiter;
pub use loudness::LoudnessMeter;

/// Tuning for one listening profile. All loudness values are K-weighted dB
/// (perceived loudness, not raw amplitude).
#[derive(Clone, Copy)]
pub struct Profile {
    /// Where perceived loudness should sit.
    pub target_db: f32,
    /// Most we will ever amplify quiet content.
    pub max_boost_db: f32,
    /// Most we will ever attenuate loud content.
    pub max_cut_db: f32,
    /// 0..1: how much of the measured distance to target we correct.
    /// 1.0 flattens everything; lower values keep some intentional dynamics.
    pub strength: f32,
    /// Below this the gain freezes, so silence and room tone are not
    /// boosted into audible hiss.
    pub gate_db: f32,
    /// How fast gain may rise (boosting quiet passages), dB per second.
    /// Kept slow so a dramatic hush is not yanked upward.
    pub rise_db_per_s: f32,
    /// How fast gain may rise while still below neutral, i.e. recovering
    /// from an earlier cut once the loud passage ends. Faster than boosting,
    /// because returning to unity gain is low-risk.
    pub recover_db_per_s: f32,
    /// How fast gain may fall (taming loud passages), dB per second.
    /// Faster, because a sudden explosion needs to come down quickly.
    pub fall_db_per_s: f32,
    /// Emergency ceiling on the fall rate. When output loudness overshoots
    /// the target badly (a loud onset landing while gain is boosted), the
    /// fall rate scales up toward this so the cut completes inside the
    /// onset transient itself, where the ear can't track it. A cut that
    /// finishes in ~50 ms reads as "that sound is loud", not "the volume
    /// just dropped".
    pub fast_fall_db_per_s: f32,
    /// Limiter ceiling (linear); 0.891 is about -1 dBFS.
    pub ceiling: f32,
    /// Limiter look-ahead in seconds. Also the processing latency.
    pub lookahead_s: f32,
}

impl Profile {
    /// Movie/TV: strong leveling so dialogue and action land close together.
    pub fn movie() -> Self {
        Self {
            target_db: -24.0,
            max_boost_db: 12.0,
            max_cut_db: 15.0,
            strength: 0.85,
            gate_db: -55.0,
            rise_db_per_s: 14.0,
            recover_db_per_s: 40.0,
            fall_db_per_s: 60.0,
            fast_fall_db_per_s: 600.0,
            ceiling: 0.891,
            lookahead_s: 0.008,
        }
    }

    /// Music: gentler correction that preserves intentional dynamics.
    pub fn music() -> Self {
        Self {
            target_db: -20.0,
            max_boost_db: 6.0,
            max_cut_db: 8.0,
            strength: 0.4,
            gate_db: -55.0,
            rise_db_per_s: 3.0,
            recover_db_per_s: 10.0,
            fall_db_per_s: 20.0,
            fast_fall_db_per_s: 20.0,
            ceiling: 0.891,
            lookahead_s: 0.008,
        }
    }
}

/// The TAN engine: perceptual loudness metering feeding a two-way automatic
/// gain rider, with a look-ahead peak limiter as the safety net.
pub struct Normalizer {
    meter: LoudnessMeter,
    limiter: Limiter,
    profile: Profile,
    channels: usize,
    gain_db: f32,
    rise_per_frame: f32,
    recover_per_frame: f32,
    fall_per_frame: f32,
    fast_fall_ratio: f32,
}

impl Normalizer {
    pub fn new(sample_rate: u32, channels: usize, profile: Profile) -> Self {
        Self {
            meter: LoudnessMeter::new(sample_rate, channels),
            limiter: Limiter::new(sample_rate, channels, profile.ceiling, profile.lookahead_s),
            profile,
            channels,
            gain_db: 0.0,
            rise_per_frame: profile.rise_db_per_s / sample_rate as f32,
            recover_per_frame: profile.recover_db_per_s / sample_rate as f32,
            fall_per_frame: profile.fall_db_per_s / sample_rate as f32,
            fast_fall_ratio: (profile.fast_fall_db_per_s / profile.fall_db_per_s).max(1.0),
        }
    }

    /// Frames of delay introduced by the look-ahead limiter.
    pub fn latency_frames(&self) -> usize {
        self.limiter.latency_frames()
    }

    /// Current gain being applied, for metering/UI.
    pub fn current_gain_db(&self) -> f32 {
        self.gain_db
    }

    /// Process interleaved samples in place. All channels share one gain so
    /// the stereo/surround image never shifts.
    pub fn process(&mut self, interleaved: &mut [f32]) {
        assert_eq!(interleaved.len() % self.channels, 0);
        let p = self.profile;
        for frame in interleaved.chunks_exact_mut(self.channels) {
            let loudness = self.meter.process_frame(frame);

            if loudness > p.gate_db {
                let desired =
                    ((p.target_db - loudness) * p.strength).clamp(-p.max_cut_db, p.max_boost_db);
                if desired > self.gain_db {
                    let step = if self.gain_db < 0.0 {
                        self.recover_per_frame
                    } else {
                        self.rise_per_frame
                    };
                    self.gain_db = (self.gain_db + step).min(desired);
                } else {
                    // How loud the *output* is right now relative to target.
                    // The bigger the overshoot, the faster we cut, so large
                    // corrections finish while the onset is still masking them
                    // and small ones stay too slow to hear as pumping.
                    let overshoot = loudness + self.gain_db - p.target_db;
                    let urgency = (overshoot / 3.0).clamp(1.0, self.fast_fall_ratio);
                    self.gain_db = (self.gain_db - self.fall_per_frame * urgency).max(desired);
                }
            }

            let gain = 10.0f32.powf(self.gain_db / 20.0);
            for sample in frame.iter_mut() {
                *sample *= gain;
            }
            self.limiter.process_frame(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, freq: f32, seconds: f32, sample_rate: u32) -> Vec<f32> {
        (0..(seconds * sample_rate as f32) as usize)
            .map(|n| {
                amplitude * (2.0 * std::f32::consts::PI * freq * n as f32 / sample_rate as f32).sin()
            })
            .collect()
    }

    fn loudness_of(samples: &[f32], sample_rate: u32) -> f32 {
        let mut meter = LoudnessMeter::new(sample_rate, 1);
        let mut db = f32::MIN;
        for s in samples {
            db = meter.process_frame(&[*s]);
        }
        db
    }

    #[test]
    fn quiet_content_gets_boosted() {
        let mut n = Normalizer::new(48000, 1, Profile::movie());
        let mut buf = sine(0.01, 997.0, 4.0, 48000);
        let before = loudness_of(&buf[96000..], 48000);
        n.process(&mut buf);
        let after = loudness_of(&buf[96000..], 48000);
        assert!(
            after - before > 8.0,
            "quiet passage should rise by ~10 dB, went {before} -> {after}"
        );
    }

    #[test]
    fn loud_content_gets_tamed() {
        let mut n = Normalizer::new(48000, 1, Profile::movie());
        let mut buf = sine(0.7, 997.0, 4.0, 48000);
        let before = loudness_of(&buf[96000..], 48000);
        n.process(&mut buf);
        let after = loudness_of(&buf[96000..], 48000);
        assert!(
            before - after > 6.0,
            "loud passage should drop substantially, went {before} -> {after}"
        );
    }

    #[test]
    fn silence_is_not_amplified() {
        let mut n = Normalizer::new(48000, 1, Profile::movie());
        let mut buf = vec![0.0f32; 48000 * 2];
        n.process(&mut buf);
        let peak = buf.iter().fold(0.0f32, |a, x| a.max(x.abs()));
        assert!(peak < 1e-6, "silence should stay silent, peak {peak}");
    }

    #[test]
    fn output_respects_ceiling() {
        let mut n = Normalizer::new(48000, 2, Profile::movie());
        let mono = sine(0.95, 60.0, 2.0, 48000);
        let mut buf: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();
        n.process(&mut buf);
        let peak = buf.iter().fold(0.0f32, |a, x| a.max(x.abs()));
        assert!(peak <= 0.891 + 1e-4, "peak {peak} exceeded ceiling");
    }

    #[test]
    fn loud_onset_is_tamed_before_the_ear_can_track_it() {
        let sr = 48000;
        let mut buf = Vec::new();
        buf.extend(sine(0.02, 300.0, 2.0, sr)); // quiet: gain rides up to a boost
        buf.extend(sine(0.6, 300.0, 2.0, sr)); // sudden loud onset
        let mut n = Normalizer::new(sr, 1, Profile::movie());
        n.process(&mut buf);

        let onset = 2 * sr as usize;
        // Loudness over a 150 ms window starting t_ms after the onset,
        // measured with a fresh meter so earlier audio can't bleed in.
        let window_at = |t_ms: usize| {
            let mut meter = LoudnessMeter::new(sr, 1);
            let start = onset + t_ms * sr as usize / 1000;
            let end = start + 150 * sr as usize / 1000;
            let mut db = f32::MIN;
            for s in &buf[start..end] {
                db = meter.process_frame(&[*s]);
            }
            db
        };
        let early = window_at(150);
        let settled = window_at(1350);
        assert!(
            (early - settled).abs() < 3.0,
            "150 ms after a loud onset the level should already be settled \
             (no audible slide): early {early}, settled {settled}"
        );
    }

    #[test]
    fn dynamic_range_is_reduced() {
        let sr = 48000;
        let mut buf = Vec::new();
        buf.extend(sine(0.02, 300.0, 3.0, sr));
        buf.extend(sine(0.6, 300.0, 3.0, sr));
        let quiet_in = loudness_of(&buf[2 * sr as usize..3 * sr as usize], sr);
        let loud_in = loudness_of(&buf[5 * sr as usize..], sr);

        let mut n = Normalizer::new(sr, 1, Profile::movie());
        n.process(&mut buf);
        let quiet_out = loudness_of(&buf[2 * sr as usize..3 * sr as usize], sr);
        let loud_out = loudness_of(&buf[5 * sr as usize..], sr);

        let range_in = loud_in - quiet_in;
        let range_out = loud_out - quiet_out;
        assert!(
            range_out < range_in * 0.5,
            "range should at least halve: {range_in} -> {range_out}"
        );
    }
}
