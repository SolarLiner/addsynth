use std::cell::{Cell, RefCell};
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
}

#[derive(Debug, Clone)]
pub struct Adsr {
    params: Arc<AdsrParams>,
    smoother: Smoother<f32>,
    state: Cell<AdsrState>,
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
            state: Cell::new( AdsrState::A),
        }
    }

    pub fn value(&self) -> f32 {
        self.smoother.previous_value()
    }

    pub fn next(&self) -> f32 {
        todo!()
    }
}

pub struct AdsrParams {
    prefix: &'static str,
    a: FloatParam,
    d: FloatParam,
    s: FloatParam,
    r: FloatParam,
}

impl fmt::Debug for AdsrParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdsrParams").finish_non_exhaustive()
    }
}

impl AdsrParams {
    pub fn new(prefix: &'static str) -> Self {
        Self {
            prefix,
            a: adr_param(format!("Attack"), 10.),
            d: adr_param(format!("Decay"), 300.),
            s: s_param(format!("Sustain"), 0.5),
            r: adr_param(format!("Release"), 300.),
        }
    }
}

unsafe impl Params for AdsrParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        vec![
            (format!("{}_a", self.prefix), self.a.as_ptr(), "".to_string()),
            (format!("{}_d", self.prefix), self.d.as_ptr(), "".to_string()),
            (format!("{}_s", self.prefix), self.s.as_ptr(), "".to_string()),
            (format!("{}_r", self.prefix), self.r.as_ptr(), "".to_string()),
        ]
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
