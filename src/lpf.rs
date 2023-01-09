use std::f32::consts::PI;

use nalgebra::{SMatrix, SVector};
use nih_plug::nih_log;
use num_complex::ComplexFloat;

use crate::math::{nr_step, ScalarField};

#[derive(Debug, Copy, Clone)]
pub struct LP1 {
    pub samplerate: f32,
    pub fc: f32,
    fb: f32,
}

impl LP1 {
    pub fn new(samplerate: f32, fc: f32) -> Self {
        Self {
            samplerate,
            fb: 0.,
            fc,
        }
    }

    fn fb_gain(&self) -> f32 {
        self.fc * PI / self.samplerate
    }

    #[inline(always)]
    pub fn process_lp(&mut self, x: f32) -> f32 {
        let in0 = self.fb_gain() * x;
        let y = in0 + self.fb;
        let y = y.tanh();
        self.fb = y - in0;
        y
    }

    #[inline(always)]
    pub fn process_hp(&mut self, x: f32) -> f32 {
        let in0 = self.fb_gain() * x;
        let yhp = self.fb + in0;
        let y = yhp.tanh();
        self.fb = y - in0;
        yhp
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LP<const N: usize>([LP1; N]);

impl<const N: usize> LP<N> {
    pub fn new(samplerate: f32, fc: f32) -> Self {
        Self([LP1::new(samplerate, fc); N])
    }

    pub fn set_samplerate(&mut self, samplerate: f32) {
        for filt in self.0.iter_mut() {
            filt.samplerate = samplerate;
        }
    }

    pub fn set_fc(&mut self, fc: f32) {
        for filt in self.0.iter_mut() {
            filt.fc = fc;
        }
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.0.iter_mut().fold(x, |s, f| f.process_lp(s))
    }
}

type Y = SVector<f32, 4>;

#[derive(Debug, Copy, Clone)]
pub struct Ladder {
    samplerate: f32,
    u: Y,
    g: f32,
    s: Y,
    k: f32,
    fb: f32,
}

impl Ladder {
    pub fn new(samplerate: f32, fc: f32, q: f32) -> Self {
        Self {
            samplerate,
            u: Y::zeros(),
            g: PI * fc / samplerate,
            s: Y::zeros(),
            k: q,
            fb: 0.,
        }
    }

    pub fn set_fc(&mut self, fc: f32) {
        self.g = PI * fc.min(self.samplerate/2.) / self.samplerate;
    }

    pub fn set_resonance(&mut self, q: f32) {
        self.k = q;
    }

    #[inline(always)]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let phi = Phi {
            g: self.g,
            k: self.k,
            s: self.s,
            x,
        };
        // self.u = phi.eval_u();
        for i in 0..4 {
            let Some(step) = nr_step(&phi, &self.u) else {
                break;
            };
            self.u -= step;
            if step.magnitude_squared() < 1e-3 {
                nih_log!("Converged after {i} iterations (mag. {} < 1e-3)", step.magnitude_squared());
                break;
            }
        }
        // for s in self.u.iter_mut() {
        //     *s = s.clamp(-1., 1.);
        // }
        let y = self.g * self.u + self.s;
        self.s = self.u;
        y[3]
    }
}

struct Phi {
    x: f32,
    g: f32,
    k: f32,
    s: Y,
}

const DIODE_PARAM: f32 = 0.2577819;
#[inline]
fn sat(x: f32) -> f32 {
    x.tanh()
    // x/(DIODE_PARAM+x.abs())
}

#[inline]
fn satd(x: f32) -> f32 {
    1. - x.tanh().powi(2)
    // DIODE_PARAM / (DIODE_PARAM + x.abs().powi(2))
}

impl Phi {
    #[inline(always)]
    fn v(&self, s: &Y) -> Y {
        Y::new(
            self.x - self.k * s[3] - s[0],
            s[0] - s[1],
            s[1] - s[2],
            s[2] - s[3],
        )
    }

    #[inline(always)]
    fn eval_u(&self) -> Y {
        self.v(&self.s).map(sat) * self.g + self.s
    }
}

impl ScalarField<f32, 4> for Phi {
    #[inline(always)]
    fn eval(&self, u: &SVector<f32, 4>) -> SVector<f32, 4> {
        self.eval_u() - u
    }

    #[inline(always)]
    fn jacobian(&self, _: &SVector<f32, 4>) -> SMatrix<f32, 4, 4> {
        -SMatrix::identity()
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use crate::lpf::Ladder;

    #[test]
    fn phi_nr() {
        const FS: f32 = 44.1e3;
        const FREQ: f32 = 500.;
        const FC: f32 = 19.8e3;
        const PERIOD: f32 = 1. / FREQ;
        let mut output = File::create("lpf.tsv").unwrap();
        let mut filter = Ladder::new(FS, FC, 1.);
        writeln!(output, "\"x\"\t\"y\"\t\"s\"").unwrap();
        for i in 0..512 {
            let t = i as f32 / FS;
            let f = (t / PERIOD).fract();

            let x = 2. * f - 1.;
            // let x = if f < 0.5 { 1. } else { -1. };
            let y = filter.process_sample(x);
            writeln!(output, "{}\t{y}\t\"{:?}\"", x, filter.s).unwrap();
        }
    }
}
