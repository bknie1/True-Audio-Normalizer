use crate::biquad::Biquad;

/// BS.1770 K-weighting filter parameters. These model how loud content
/// actually *sounds* to a human rather than raw sample amplitude: a shelf
/// boost in the presence region (head-related emphasis) plus a high-pass
/// that discounts low rumble the ear is insensitive to.
const SHELF_FREQ: f64 = 1681.974450955533;
const SHELF_GAIN_DB: f64 = 3.99984385397905;
const SHELF_Q: f64 = 0.7071752369554196;
const HIGHPASS_FREQ: f64 = 38.13547087602444;
const HIGHPASS_Q: f64 = 0.5003270373238773;

struct KWeight {
    shelf: Biquad,
    highpass: Biquad,
}

impl KWeight {
    fn new(sample_rate: f64) -> Self {
        Self {
            shelf: Biquad::high_shelf(sample_rate, SHELF_FREQ, SHELF_GAIN_DB, SHELF_Q),
            highpass: Biquad::high_pass(sample_rate, HIGHPASS_FREQ, HIGHPASS_Q),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        self.highpass.process(self.shelf.process(x))
    }
}

/// Tracks perceived loudness of a running signal, in dB (K-weighted, roughly
/// LUFS-like). Channels are measured together so a loud event in any channel
/// registers once.
pub struct LoudnessMeter {
    filters: Vec<KWeight>,
    mean_square: f32,
    attack_coef: f32,
    release_coef: f32,
}

/// One-pole smoothing coefficient for a given time constant.
pub(crate) fn smoothing_coef(time_constant_s: f32, sample_rate: f32) -> f32 {
    (-1.0 / (time_constant_s * sample_rate)).exp()
}

impl LoudnessMeter {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self {
            filters: (0..channels)
                .map(|_| KWeight::new(sample_rate as f64))
                .collect(),
            mean_square: 0.0,
            // React quickly when things get loud, back off slowly when quiet,
            // so brief pauses in speech don't read as "the scene went quiet".
            attack_coef: smoothing_coef(0.030, sample_rate as f32),
            release_coef: smoothing_coef(0.400, sample_rate as f32),
        }
    }

    /// Feed one frame (one sample per channel); returns current loudness in dB.
    pub fn process_frame(&mut self, frame: &[f32]) -> f32 {
        let mut energy = 0.0f32;
        for (sample, filter) in frame.iter().zip(self.filters.iter_mut()) {
            let weighted = filter.process(*sample);
            energy += weighted * weighted;
        }
        energy /= self.filters.len() as f32;

        let coef = if energy > self.mean_square {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.mean_square = coef * self.mean_square + (1.0 - coef) * energy;

        self.loudness_db()
    }

    pub fn loudness_db(&self) -> f32 {
        // -0.691 is the BS.1770 calibration offset; the tiny floor keeps
        // log10 finite during silence.
        -0.691 + 10.0 * (self.mean_square + 1e-12).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure_sine(amplitude: f32, freq: f32) -> f32 {
        let mut meter = LoudnessMeter::new(48000, 1);
        let mut db = f32::MIN;
        for n in 0..(48000 * 2) {
            let x = amplitude * (2.0 * std::f32::consts::PI * freq * n as f32 / 48000.0).sin();
            db = meter.process_frame(&[x]);
        }
        db
    }

    #[test]
    fn amplitude_ratio_matches_db_difference() {
        let loud = measure_sine(0.5, 997.0);
        let quiet = measure_sine(0.05, 997.0);
        let diff = loud - quiet;
        assert!(
            (diff - 20.0).abs() < 1.0,
            "10x amplitude should measure ~20 dB apart, got {diff}"
        );
    }

    #[test]
    fn rumble_counts_less_than_midrange() {
        let mid = measure_sine(0.25, 1000.0);
        let rumble = measure_sine(0.25, 25.0);
        assert!(
            mid - rumble > 10.0,
            "25 Hz rumble should register much quieter than 1 kHz, got mid={mid} rumble={rumble}"
        );
    }

    #[test]
    fn silence_reads_very_quiet() {
        let mut meter = LoudnessMeter::new(48000, 2);
        let mut db = 0.0;
        for _ in 0..48000 {
            db = meter.process_frame(&[0.0, 0.0]);
        }
        assert!(db < -90.0, "silence should read below -90 dB, got {db}");
    }
}
