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
    y: Y,
    k: f32,
    fb: f32,
}

impl Ladder {
    pub fn new(samplerate: f32, fc: f32, q: f32) -> Self {
        Self {
            samplerate,
            u: Y::zeros(),
            g: PI * fc / samplerate,
            y: Y::zeros(),
            k: q,
            fb: 0.,
        }
    }

    pub fn set_fc(&mut self, fc: f32) {
        self.g = PI * fc.min(self.samplerate) / self.samplerate;
    }

    pub fn set_resonance(&mut self, q: f32) {
        self.k = q;
    }

    #[inline(always)]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let phi = Phi {
            g: self.g,
            k: self.k,
            s: self.y,
            x,
        };
        for i in 0..4 {
            let Some(step) = nr_step(&phi, &self.y) else {
                break;
            };
            self.y -= step;
            if step.magnitude_squared() < 1e-4 {
                // nih_log!("Converged after {i} iterations (mag. {} < 1e-4)", step.magnitude_squared());
                break;
            }
        }
        self.u = phi.eval_u(&self.y);
        self.y[3]
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
    fn v(&self, y: &Y) -> Y {
        Y::new(
            self.x - self.k * y[3] - y[0],
            y[0] - y[1],
            y[1] - y[2],
            y[2] - y[3],
        )
    }

    #[inline(always)]
    fn eval_u(&self, y: &Y) -> Y {
        self.v(y).map(sat) * self.g
    }
}

impl ScalarField<f32, 4> for Phi {
    #[inline(always)]
    fn eval(&self, y: &SVector<f32, 4>) -> SVector<f32, 4> {
        self.eval_u(y) + self.s - y
    }

    #[inline(always)]
    #[rustfmt::skip]
    fn jacobian(&self, y: &SVector<f32, 4>) -> SMatrix<f32, 4, 4> {
        let v = self.v(y);
        let v = v.map(satd);
        SMatrix::<_, 4, 4>::new(
            // Row 1
            -v[0], 0., 0., -self.k * v[0],
            // Row 2
            v[1], -v[1], 0., 0.,
            // Row 3
            0., v[2], -v[2], 0.,
            // Row 4
            0., 0., v[3], -v[3],
        ) * self.g - SMatrix::identity()
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
        const FC: f32 = 6e3;
        const PERIOD: f32 = 1. / FREQ;
        let mut output = File::create("lpf.tsv").unwrap();
        let mut filter = Ladder::new(FS, FC, 8.);
        writeln!(output, "\"x\"\t\"y\"\t\"s\"").unwrap();
        for i in 0..512 {
            let t = i as f32 / FS;
            let f = (t / PERIOD).fract();

            let x = 2. * f - 1.;
            // let x = if f < 0.5 { 1. } else { -1. };
            let y = filter.process_sample(x);
            writeln!(output, "{}\t{y}\t\"{:?}\"", x, filter.y).unwrap();
        }
    }
}
