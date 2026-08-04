// Rapid Text DSP Audio Filter Shield
// 150Hz 2nd-Order Butterworth IIR High-Pass Filter + Soft Peak Limiter + Noise Gate

use std::f32::consts::PI;

/// 2nd-Order Butterworth High-Pass Biquad Filter
/// Removes sub-vocal rumble (<150Hz), desk thumps, HVAC hum, and sub-bass adversarial anti-AI noise.
#[derive(Debug, Clone)]
pub struct BiquadHpf150 {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // Filter history (states) per channel
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadHpf150 {
    /// Create a new 150Hz HPF for the specified sample rate.
    pub fn new(sample_rate: u32) -> Self {
        let cutoff_hz = 150.0f32;
        let sr = sample_rate as f32;
        let omega = 2.0 * PI * cutoff_hz / sr;
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha = sin_w / (2.0 * 0.70710678f32); // Q = sqrt(0.5) for Butterworth

        let a0 = 1.0 + alpha;
        let b0 = ((1.0 + cos_w) / 2.0) / a0;
        let b1 = (-(1.0 + cos_w)) / a0;
        let b2 = ((1.0 + cos_w) / 2.0) / a0;
        let a1 = (-2.0 * cos_w) / a0;
        let a2 = (1.0 - alpha) / a0;

        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Filter a single sample in-place.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// Process a slice of f32 samples in-place.
    pub fn process_slice(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Reset filter state history.
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Unified DSP Audio Pipeline
#[derive(Debug, Clone)]
pub struct AudioDspPipeline {
    hpf: BiquadHpf150,
}

impl AudioDspPipeline {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            hpf: BiquadHpf150::new(sample_rate),
        }
    }

    /// Process a chunk of PCM audio in-place:
    /// 1. Apply 150Hz High-Pass Filter (strips sub-bass rumble & plosives)
    /// 2. Apply Noise Gate & Soft Peak Limiter (tanh ceiling)
    pub fn process_in_place(&mut self, samples: &mut [f32], noise_gate_threshold: f32) {
        // 1. High-pass filter
        self.hpf.process_slice(samples);

        // 2. Noise gate + Soft limiter
        let inv_threshold = if noise_gate_threshold > 0.0 {
            1.0 / noise_gate_threshold
        } else {
            0.0
        };

        for s in samples.iter_mut() {
            // Apply noise gate
            let abs = s.abs();
            if noise_gate_threshold > 0.0 && abs < noise_gate_threshold {
                let ratio = abs * inv_threshold;
                *s *= ratio.cbrt();
            }

            // Apply soft peak limiter (tanh ceiling at 1.0) with gain boost
            let val = *s * 1.5;
            if val.abs() > 0.95 {
                *s = val.signum() * (1.0 - (-val.abs()).exp());
            } else {
                *s = val;
            }
        }
    }
}
