use std::f32::{consts::TAU, EPSILON};

use crate::phasor::Phasor;

#[derive(Debug, Clone, Copy)]
pub struct Oscillator {
    pub partials: [(f32, Phasor); 1024],
}

impl Default for Oscillator {
    fn default() -> Self {
        Self {
            partials: [0.0; 1024].map(|z| (z, Phasor::new(0.0, z))),
        }
    }
}

impl Oscillator {
    pub fn sine(samplerate: f32, hz: f32) -> Self {
        let mut this = Self::default();
        this.partials[0].0 = 1.0;
        this.partials[0].1 = Phasor::new(samplerate, hz);
        this
    }

    pub fn triangle(samplerate: f32, hz: f32) -> Self {
        Self {
            partials: std::array::from_fn(|i| {
                let i = i + 1;
                let gain = 1.0 / i.pow(2) as f32;
                (gain, Phasor::new(samplerate, hz * (2.0 * i as f32 - 1.0)))
            })
        }
    }

    pub fn square(samplerate: f32, hz: f32) -> Self {
        Self {
            partials: std::array::from_fn(|i| {
                let i = i + 1;
                let inc = 2.0 * i as f32 - 1.0;
                let gain = 1.0 / inc;
                (gain, Phasor::new(samplerate, hz * inc))
            })
        }
    }

    pub fn saw(samplerate: f32, hz: f32) -> Self {
        Self {
            partials: std::array::from_fn(|i| {
                (1.0 / (1.0 + i as f32), Phasor::new(samplerate, hz * i as f32))
            })
        }
    }

    pub fn sample(&mut self) -> f32 {
        let nyquist = self.samplerate() / 2.0;
        self.partials
            .iter_mut()
            .filter(|(gain, p)| *gain > EPSILON && p.hz < nyquist)
            .map(|(gain, p)| (TAU * p.inc(1)).sin() * *gain)
            .sum::<f32>() / 2.0
    }

    pub fn samplerate(&self) -> f32 {
        self.partials[0].1.samplerate
    }

    pub fn set_frequency(&mut self, hz: f32) {
        for (_, partial) in self.partials.iter_mut() {
            partial.hz = hz;
        }
    }

    pub fn set_phase(&mut self, phase: f32) {
        for (_, partial) in self.partials.iter_mut() {
            partial.set_phase(phase);
        }
    }
}
