mod biquad;
mod limiter;
mod loudness;
mod offline;

pub use limiter::Limiter;
pub use loudness::LoudnessMeter;
pub use offline::normalize_offline;

/// Tuning for one listening profile. All loudness values are K-weighted dB
/// (perceived loudness, not raw amplitude).
#[derive(Clone, Copy)]
pub struct Profile {
    /// The engine levels around the content's own self-measured baseline
    /// rather than any absolute target, so overall perceived volume always
    /// matches the source. Perception anchors on the prominent material, so
    /// the baseline follows louder content quickly (this many seconds)...
    pub baseline_rise_s: f32,
    /// ...and sinks toward quieter content much more slowly, keeping it
    /// parked near the loud level. Loud passages barely move; quiet
    /// dialogue rises to meet them.
    pub baseline_sink_s: f32,
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
    /// Offline (two-pass) only: how fast gain may ramp down *ahead of* a
    /// loud onset it can see coming. Quick, so it finishes just before the
    /// onset without audibly ducking the tail of the preceding content.
    pub preduck_db_per_s: f32,
    /// Limiter ceiling (linear); 0.891 is about -1 dBFS.
    pub ceiling: f32,
    /// Limiter look-ahead in seconds. Also the processing latency.
    pub lookahead_s: f32,
}

impl Profile {
    /// Movie/TV: strong leveling so dialogue and action land close together.
    pub fn movie() -> Self {
        Self {
            baseline_rise_s: 1.5,
            baseline_sink_s: 20.0,
            max_boost_db: 12.0,
            max_cut_db: 15.0,
            strength: 0.85,
            gate_db: -55.0,
            rise_db_per_s: 14.0,
            recover_db_per_s: 40.0,
            fall_db_per_s: 60.0,
            fast_fall_db_per_s: 600.0,
            preduck_db_per_s: 150.0,
            ceiling: 0.891,
            lookahead_s: 0.008,
        }
    }

    /// Music: gentler correction that preserves intentional dynamics.
    pub fn music() -> Self {
        Self {
            baseline_rise_s: 4.0,
            baseline_sink_s: 30.0,
            max_boost_db: 6.0,
            max_cut_db: 8.0,
            strength: 0.4,
            gate_db: -55.0,
            rise_db_per_s: 3.0,
            recover_db_per_s: 10.0,
            fall_db_per_s: 20.0,
            fast_fall_db_per_s: 20.0,
            preduck_db_per_s: 40.0,
            ceiling: 0.891,
            lookahead_s: 0.008,
        }
    }

    /// Universal: the safe catch-all when the content type is unknown.
    /// Moderate leveling that sits between Movie and Music - noticeable help
    /// on badly mixed material without flattening deliberately dynamic content.
    pub fn universal() -> Self {
        Self {
            baseline_rise_s: 2.5,
            baseline_sink_s: 25.0,
            max_boost_db: 9.0,
            max_cut_db: 12.0,
            strength: 0.6,
            gate_db: -55.0,
            rise_db_per_s: 8.0,
            recover_db_per_s: 25.0,
            fall_db_per_s: 40.0,
            fast_fall_db_per_s: 400.0,
            preduck_db_per_s: 100.0,
            ceiling: 0.891,
            lookahead_s: 0.008,
        }
    }

    /// Speech / podcast: maximize dialogue intelligibility. Brings quiet
    /// talking up firmly and follows the voice quickly, with a slightly higher
    /// gate so breaths and room tone in the gaps are not pumped up.
    pub fn speech() -> Self {
        Self {
            baseline_rise_s: 1.0,
            baseline_sink_s: 15.0,
            max_boost_db: 14.0,
            max_cut_db: 12.0,
            strength: 0.8,
            gate_db: -50.0,
            rise_db_per_s: 10.0,
            recover_db_per_s: 30.0,
            fall_db_per_s: 45.0,
            fast_fall_db_per_s: 300.0,
            preduck_db_per_s: 120.0,
            ceiling: 0.891,
            lookahead_s: 0.008,
        }
    }

    /// Night: aggressive dynamic-range compression for quiet hours. Pulls
    /// quiet content up and slams loud spikes down hard and fast, with extra
    /// peak headroom, so nothing wakes the house.
    pub fn night() -> Self {
        Self {
            baseline_rise_s: 1.0,
            baseline_sink_s: 12.0,
            max_boost_db: 12.0,
            max_cut_db: 24.0,
            strength: 0.95,
            gate_db: -60.0,
            rise_db_per_s: 12.0,
            recover_db_per_s: 40.0,
            fall_db_per_s: 90.0,
            fast_fall_db_per_s: 900.0,
            preduck_db_per_s: 200.0,
            ceiling: 0.708, // ~ -3 dBFS, quieter peaks
            lookahead_s: 0.008,
        }
    }

    /// Game: preserve punch and positional cues while taming extremes. A
    /// lighter touch than Movie so transients and dynamics that carry
    /// gameplay information survive.
    pub fn game() -> Self {
        Self {
            baseline_rise_s: 2.0,
            baseline_sink_s: 25.0,
            max_boost_db: 8.0,
            max_cut_db: 10.0,
            strength: 0.5,
            gate_db: -55.0,
            rise_db_per_s: 6.0,
            recover_db_per_s: 20.0,
            fall_db_per_s: 35.0,
            fast_fall_db_per_s: 350.0,
            preduck_db_per_s: 90.0,
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
    baseline_db: Option<f32>,
    baseline_rise_coef: f32,
    baseline_sink_coef: f32,
    baseline_fast_coef: f32,
    /// While counting down, the baseline chases the signal quickly - it
    /// locks onto the content's real level within a couple of seconds of
    /// audible material (covering meter warmup and enable-mid-stream),
    /// then switches to the slow drift.
    lock_frames_left: usize,
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
            baseline_db: None,
            baseline_rise_coef: loudness::smoothing_coef(profile.baseline_rise_s, sample_rate as f32),
            baseline_sink_coef: loudness::smoothing_coef(profile.baseline_sink_s, sample_rate as f32),
            baseline_fast_coef: loudness::smoothing_coef(0.5, sample_rate as f32),
            lock_frames_left: 2 * sample_rate as usize,
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

    /// Set per-channel loudness-measurement weights from the speaker layout
    /// (one per channel). Lets the gain rider follow, say, the center/dialogue
    /// channel while discounting LFE and surround/height. No-op if the length
    /// doesn't match the channel count. Stereo/mono callers can ignore this.
    pub fn set_channel_weights(&mut self, weights: &[f32]) {
        self.meter.set_weights(weights);
    }

    /// Process interleaved samples in place. All channels share one gain so
    /// the stereo/surround image never shifts.
    pub fn process(&mut self, interleaved: &mut [f32]) {
        assert_eq!(interleaved.len() % self.channels, 0);
        let p = self.profile;
        for frame in interleaved.chunks_exact_mut(self.channels) {
            let loudness = self.meter.process_frame(frame);

            if loudness > p.gate_db {
                // The baseline is the content's own slowly-tracked average
                // loudness; leveling happens around it, never toward an
                // absolute number, so overall volume stays where the source
                // put it. It locks to the first audible material instantly,
                // then drifts with the program.
                let baseline = match self.baseline_db {
                    None => {
                        self.baseline_db = Some(loudness);
                        loudness
                    }
                    Some(prev) => {
                        let coef = if self.lock_frames_left > 0 {
                            self.lock_frames_left -= 1;
                            self.baseline_fast_coef
                        } else if loudness > prev {
                            self.baseline_rise_coef
                        } else {
                            self.baseline_sink_coef
                        };
                        let b = coef * prev + (1.0 - coef) * loudness;
                        self.baseline_db = Some(b);
                        b
                    }
                };

                let desired =
                    ((baseline - loudness) * p.strength).clamp(-p.max_cut_db, p.max_boost_db);
                if desired > self.gain_db {
                    let step = if self.gain_db < 0.0 {
                        self.recover_per_frame
                    } else {
                        self.rise_per_frame
                    };
                    self.gain_db = (self.gain_db + step).min(desired);
                } else {
                    // How loud the *output* is right now relative to the
                    // baseline. The bigger the overshoot, the faster we cut,
                    // so large corrections finish while the onset is still
                    // masking them and small ones stay too slow to hear as
                    // pumping.
                    let overshoot = loudness + self.gain_db - baseline;
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

    /// Uniform content is already at its own baseline; TAN must not
    /// re-level it. This is what keeps overall volume identical to the
    /// source when the material doesn't need help.
    #[test]
    fn steady_content_is_left_alone() {
        for amplitude in [0.05, 0.5] {
            let mut n = Normalizer::new(48000, 1, Profile::movie());
            let mut buf = sine(amplitude, 997.0, 4.0, 48000);
            let before = loudness_of(&buf[96000..], 48000);
            n.process(&mut buf);
            let after = loudness_of(&buf[96000..], 48000);
            assert!(
                (after - before).abs() < 1.5,
                "steady content at amp {amplitude} should pass through, went {before} -> {after}"
            );
        }
    }

    fn alternating(sr: u32) -> Vec<f32> {
        let mut buf = Vec::new();
        for block in 0..4 {
            let amp = if block % 2 == 0 { 0.02 } else { 0.6 };
            buf.extend(sine(amp, 300.0, 3.0, sr));
        }
        buf
    }

    /// Quiet passages rise toward the content's own average and loud ones
    /// fall toward it - leveling around the baseline, not toward any
    /// absolute target.
    #[test]
    fn mixed_content_is_leveled_around_its_baseline() {
        let sr = 48000;
        let mut buf = alternating(sr);
        let quiet_in = loudness_of(&buf[8 * sr as usize..9 * sr as usize], sr);
        let loud_in = loudness_of(&buf[11 * sr as usize..], sr);
        let mut n = Normalizer::new(sr, 1, Profile::movie());
        n.process(&mut buf);
        let quiet_out = loudness_of(&buf[8 * sr as usize..9 * sr as usize], sr);
        let loud_out = loudness_of(&buf[11 * sr as usize..], sr);
        assert!(
            quiet_out - quiet_in > 8.0,
            "established quiet should rise to meet the baseline: {quiet_in} -> {quiet_out}"
        );
        assert!(
            loud_out < loud_in + 1.0,
            "established loud must not rise: {loud_in} -> {loud_out}"
        );
        let range_in = loud_in - quiet_in;
        let range_out = loud_out - quiet_out;
        assert!(
            range_out < range_in * 0.6,
            "range should shrink substantially: {range_in} -> {range_out}"
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
        // Settled point is close by on purpose: the baseline itself drifts
        // slowly during a sustained loud passage (by design), so we compare
        // against 600 ms rather than seconds later.
        let early = window_at(150);
        let settled = window_at(600);
        // One-sided on purpose: the artifact that must never happen is the
        // level audibly *dropping* after the onset. Gentle upward release as
        // sustained loudness becomes the new baseline is normal compressor
        // behavior.
        assert!(
            early < settled + 3.0,
            "level must not slide down after a loud onset: early {early}, settled {settled}"
        );
    }

    /// The user-facing contract behind the adaptive baseline: turning TAN
    /// on must not make content quieter (or louder) overall.
    #[test]
    fn overall_volume_is_preserved() {
        let sr = 48000;
        let mut buf = alternating(sr);
        // Preservation is a steady-state property: measure the second half,
        // after the baseline has locked onto the content.
        let mean_of = |samples: &[f32]| -> f32 {
            let mut meter = LoudnessMeter::new(sr, 1);
            let mut readings = Vec::new();
            for (i, s) in samples.iter().enumerate() {
                let db = meter.process_frame(&[*s]);
                if i % 4800 == 0 && db > -55.0 && i > 6 * sr as usize {
                    readings.push(db);
                }
            }
            readings.iter().sum::<f32>() / readings.len() as f32
        };
        let before = mean_of(&buf);
        let mut n = Normalizer::new(sr, 1, Profile::movie());
        n.process(&mut buf);
        let after = mean_of(&buf);
        assert!(
            (after - before).abs() < 2.5,
            "overall loudness should be preserved: {before} -> {after}"
        );
    }
}
