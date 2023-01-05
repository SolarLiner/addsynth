
use std::fmt;
use std::fmt::Formatter;
use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum AdsrState {
    A,
    D,
    S,
    R,
    Released,
}

#[derive(Debug, Clone)]
pub struct Adsr {
    params: Arc<AdsrParams>,
    smoother: Smoother<f32>,
    state: AdsrState,
    samplerate: f32,
}

impl Adsr {
    pub fn new(samplerate: f32, params: Arc<AdsrParams>) -> Self {
        let smoother = Smoother::new(SmoothingStyle::Exponential(params.a.value()));
        smoother.reset(0.);
        smoother.set_target(samplerate, 1.);
        Self {
            params,
            smoother,
            samplerate,
            state: AdsrState::A,
        }
    }

    pub fn value(&self) -> f32 {
        self.smoother.previous_value()
    }

    pub fn next(&mut self) -> f32 {
        if self.smoother.steps_left() == 0 {
            match self.state {
                AdsrState::A => {
                    self.smoother = Smoother::new(SmoothingStyle::Exponential(self.params.d.value()));
                    self.smoother.reset(1.0);
                    self.smoother.set_target(self.samplerate, self.params.s.value());
                    self.state = AdsrState::D;
                }
                AdsrState::D => {
                    self.state = AdsrState::S;
                }
                AdsrState::R => {
                    self.state = AdsrState::Released;
                }
                _ => {}
            }
        } else {
            match self.state {
                AdsrState::A => {
                    self.smoother.style = SmoothingStyle::Exponential(self.params.a.value());
                }
                AdsrState::D => {
                    self.smoother.style = SmoothingStyle::Exponential(self.params.d.value());
                    self.smoother.set_target(self.samplerate, self.params.s.value());
                }
                AdsrState::R => {
                    self.smoother.style = SmoothingStyle::Exponential(self.params.r.value());
                }
                _ => {}
            }
        }

        self.smoother.next()
    }

    pub fn release(&mut self) {
        let val = self.smoother.previous_value();
        self.state = AdsrState::R;
        self.smoother = Smoother::new(SmoothingStyle::Exponential(self.params.r.value()));
        self.smoother.reset(val);
        self.smoother.set_target(self.samplerate, 0.);
    }

    pub fn releasing(&self) -> bool {
        matches!(self.state, AdsrState::R)
    }

    #[inline(always)]
    pub fn active(&self) -> bool {
        match self.state {
            AdsrState::Released => false,
            AdsrState::S if self.params.s.value() == 0. => false,
            _ => true,
        }
    }
}

#[derive(Params)]
pub struct AdsrParams {
    #[id="a"]
    a: FloatParam,
    #[id="d"]
    d: FloatParam,
    #[id="s"]
    s: FloatParam,
    #[id="r"]
    r: FloatParam,
}

impl fmt::Debug for AdsrParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdsrParams").finish_non_exhaustive()
    }
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self {
            a: adr_param(format!("Attack"), 10.),
            d: adr_param(format!("Decay"), 300.),
            s: s_param(format!("Sustain"), 0.5),
            r: adr_param(format!("Release"), 300.),
        }
    }
}

fn s_param(name: impl ToString, default: f32) -> FloatParam {
    FloatParam::new(name.to_string(), default, FloatRange::Linear { min: 0., max: 1. })
        .with_string_to_value(formatters::s2v_f32_percentage())
        .with_value_to_string(formatters::v2s_f32_percentage(2))
}

fn adr_param(name: impl ToString, default: f32) -> FloatParam {
    FloatParam::new(name.to_string(), default, FloatRange::Linear { min: 0., max: 5e3 })
        .with_unit("ms")
}
