use std::simd::{f32x8, u8x8, SimdPartialOrd};

#[derive(Debug, Clone, Copy)]
pub struct Phasor8 {
    pub hz: f32x8,
    pub samplerate: f32x8,
    pub phase: f32x8,
}

impl Phasor8 {
    pub fn new(samplerate: impl Into<f32x8>, hz: impl Into<f32x8>) -> Self {
        Self {
            hz: hz.into(),
            samplerate: samplerate.into(),
            phase: f32x8::default(),
        }
    }

    pub fn step(&self) -> f32x8 {
        self.hz / self.samplerate
    }

    pub fn inc(&mut self, amt: u8x8) -> f32x8 {
        self.set_phase(self.phase + amt.cast::<f32>() * self.step())
    }

    pub fn set_phase(&mut self, phase: f32x8) -> f32x8 {
        let neg_mask = phase.simd_le(f32x8::default());
        self.phase = neg_mask.select(phase + f32x8::splat(1.), phase) % f32x8::splat(1.);
        self.phase
    }
}
