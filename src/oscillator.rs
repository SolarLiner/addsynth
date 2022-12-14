use std::array;
use std::f32::{consts::TAU, EPSILON};

use std::simd::{f32x8, mask32x8, u32x8};
use std::simd::{SimdFloat, SimdPartialOrd};

use crate::externs::SimdTrig;
use crate::phasor::Phasor8;

#[derive(Debug, Clone, Copy)]
pub struct Oscillator {
    pub phase_offset: f32,
    pub(crate) samplerate: f32,
    pub gains: [f32x8; 128],
    pub phases: [Phasor8; 128],
}

impl Oscillator {
    pub fn new(samplerate: f32) -> Self {
        Self {
            phase_offset: 0.,
            samplerate,
            gains: array::from_fn(|_| f32x8::splat(0.)),
            phases: array::from_fn(|_| Phasor8::new(f32x8::splat(samplerate), f32x8::splat(0.))),
        }
    }
    pub fn from_bode(samplerate: f32, f: impl Fn(usize) -> (f32, f32)) -> Self {
        let mut this = Self::new(samplerate);

        let samplerate = f32x8::splat(samplerate);
        let mut gains = [0.0; 1024];
        let mut frequencies = [0.0; 1024];

        for i in 0..1024 {
            let (f, p) = f(i);
            gains[i] = f;
            frequencies[i] = p;
        }

        for (i, (freqs, phases)) in gains.chunks(8).zip(frequencies.chunks(8)).enumerate() {
            this.gains[i] = f32x8::from_slice(freqs);
            this.phases[i] = Phasor8::new(samplerate, f32x8::from_slice(phases));
        }

        this
    }
    pub fn sine(samplerate: f32, hz: f32) -> Self {
        let mut this = Self::new(samplerate);
        let mask = mask32x8::from_array([true, false, false, false, false, false, false, false]);
        this.gains[0] = mask.select(f32x8::splat(1.0), f32x8::default());
        this.phases[0].hz = mask.select(f32x8::splat(hz), f32x8::default());
        this
    }

    pub fn triangle(samplerate: f32, hz: f32) -> Self {
        Self::from_bode(samplerate, |i| {
            let i = i + 1;
            let gain = f32::recip(i.pow(2) as f32);
            let freq = hz * (2.0 * i as f32 - 1.0);
            (gain, freq)
        })
    }

    pub fn square(samplerate: f32, hz: f32) -> Self {
        Self::from_bode(samplerate, |i| {
            let i = i + 1;
            let inc = 2.0 * i as f32 - 1.;
            let gain = inc.recip();
            let freq = hz * inc;
            (gain, freq)
        })
    }

    pub fn saw(samplerate: f32, hz: f32) -> Self {
        Self::from_bode(samplerate, |i| (f32::recip(1.0 + i as f32), hz * i as f32))
    }

    pub fn sample(&mut self) -> f32 {
        let nyquist = self.samplerate / 2.0;
        let phase_offset = f32x8::splat(self.phase_offset);
        self.gains
            .iter()
            .copied()
            .zip(self.phases.iter_mut())
            .filter(|(g, p)| {
                g.simd_ge(f32x8::splat(EPSILON)).any() && p.hz.simd_lt(f32x8::splat(nyquist)).any()
            })
            .map(|(gain, phase)| {
                let mask =
                    gain.simd_ge(f32x8::splat(EPSILON)) & phase.hz.simd_lt(f32x8::splat(nyquist));
                let r = gain
                    * (f32x8::splat(TAU)
                        * (phase.inc(mask.select(u32x8::splat(1), u32x8::splat(0)).cast())
                            + phase_offset))
                        .sin();
                mask.select(r, f32x8::splat(0.))
            })
            .reduce(|a, b| a + b)
            .map(|s| s.reduce_sum())
            .unwrap_or_default()
            / 2.
    }
}
