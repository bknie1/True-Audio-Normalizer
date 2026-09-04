/// A second-order IIR filter ("biquad"), the workhorse of audio EQ.
/// Coefficients follow the standard RBJ Audio EQ Cookbook formulas.
/// State is kept in f64 to avoid accumulating rounding error at low frequencies.
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    s1: f64,
    s2: f64,
}

impl Biquad {
    pub fn high_pass(sample_rate: f64, freq: f64, q: f64) -> Self {
        let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self::normalized(b0, b1, b2, a0, a1, a2)
    }

    pub fn high_shelf(sample_rate: f64, freq: f64, gain_db: f64, q: f64) -> Self {
        let a = 10.0_f64.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * freq / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;

        Self::normalized(b0, b1, b2, a0, a1, a2)
    }

    fn normalized(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// Process one sample (transposed direct form II).
    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_pass_removes_dc() {
        let mut hp = Biquad::high_pass(48000.0, 38.0, 0.5);
        let mut last = 1.0f32;
        for _ in 0..48000 {
            last = hp.process(1.0);
        }
        assert!(
            last.abs() < 0.01,
            "constant input should decay to ~0, got {last}"
        );
    }

    #[test]
    fn high_pass_passes_high_frequency() {
        let mut hp = Biquad::high_pass(48000.0, 38.0, 0.5);
        let mut peak = 0.0f32;
        for n in 0..48000 {
            let x = (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / 48000.0).sin();
            let y = hp.process(x);
            if n > 4800 {
                peak = peak.max(y.abs());
            }
        }
        assert!(
            (peak - 1.0).abs() < 0.05,
            "1 kHz should pass nearly unchanged, peak was {peak}"
        );
    }
}
