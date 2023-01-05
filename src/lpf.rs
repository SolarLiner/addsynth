use std::f32::consts::PI;
use std::ops::Sub;

use nalgebra::SVector;
use num_complex::ComplexFloat;

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

    pub fn process_sample(&mut self, x: f32) -> f32 {
        let in0 = self.fb_gain() * x;
        let y = self.fb + in0;
        let y = y.tanh();
        self.fb = y + in0;
        y
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
        self.0.iter_mut().fold(x, |s, f| f.process_sample(s))
    }
}

type F<T> = fn(T) -> T;
type Y = SVector<f32, 4>;
type L = F<Y>;

#[derive(Debug, Copy, Clone)]
pub struct Ladder {
    w_step: f32,
    y: Y,
    g: f32,
    s: Y,
    k: f32,
    fb: f32,
}

impl Ladder {
    pub fn new(samplerate: f32, fc: f32, q: f32) -> Self {
        Self {
            w_step: PI / samplerate,
            y: Y::zeros(),
            g: PI * fc / samplerate,
            s: Y::zeros(),
            k: q,
            fb: 0.,
        }
    }

    pub fn set_fc(&mut self, fc: f32) {
        self.g = self.w_step * fc;
    }

    pub fn set_resonance(&mut self, q: f32) {
        self.k = q;
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        for _ in 0..4 {
            self.y -= (self.phi(x, self.y) - self.y).component_mul(
                &self
                    .phid(self.y)
                    .map(|x| x.recip())
                    .sub(&Y::from_element(1.)),
            )
        }

        self.y[3]
    }

    fn phi(&self, x: f32, y: Y) -> Y {
        let vin = Y::new(
            x - self.k * y[3] - y[0],
            y[0] - y[1],
            y[1] - y[2],
            y[2] - y[3],
        );
        let v = vin.map(|x| x.tanh());
        self.g * v + self.s
    }

    fn phid(&self, y: Y) -> Y {
        let vin = Y::new(-1., 1., 0., 0.)
            + Y::new(0., -1., 1., 0.)
            + Y::new(0., 0., -1., 1.)
            + Y::new(-self.k, 0., 0., -1.);
        (Y::from_element(1.)
            - vin
                .map(|x| x.tanh())
                .component_mul(&y.map(|x| 1. - x.tanh().powi(2))))
            * self.g
    }
}
