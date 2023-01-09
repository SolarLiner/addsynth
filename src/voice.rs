use std::sync::{
    atomic::{AtomicPtr, AtomicU64, Ordering::Relaxed},
    Arc,
};

use nih_plug::prelude::*;

use crate::lpf::Ladder;
use crate::{
    adsr::{Adsr, AdsrParams},
    oscillator::Oscillator,
    tanh::TanhLut,
};

static NEXT_VOICE_ID: AtomicU64 = AtomicU64::new(0);

pub static TANH_LUT_PTR: AtomicPtr<TanhLut<true>> = AtomicPtr::new(std::ptr::null_mut());

/// Compute a voice ID in case the host doesn't provide them. Polyphonic modulation will not work in
/// this case, but playing notes will.
const fn compute_fallback_voice_id(note: u8, channel: u8) -> i32 {
    note as i32 | ((channel as i32) << 16)
}

#[derive(Debug, Clone, Copy)]
pub struct VoiceId {
    pub id: u64,
    pub voice_id: i32,
    pub channel: u8,
    pub note: u8,
}

impl VoiceId {
    pub fn new(voice_id: Option<i32>, channel: u8, note: u8) -> Self {
        let id = NEXT_VOICE_ID.fetch_add(1, Relaxed);
        Self {
            id,
            voice_id: voice_id.unwrap_or_else(|| compute_fallback_voice_id(note, channel)),
            channel,
            note,
        }
    }

    pub fn is_id(&self, id: i32) -> bool {
        self.voice_id == id
    }

    pub fn is_channel_note(&self, channel: u8, note: u8) -> bool {
        self.channel == channel && self.note == note
    }

    pub fn next_id() -> u64 {
        NEXT_VOICE_ID.load(Relaxed)
    }
}

impl PartialEq for VoiceId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for VoiceId {}

#[derive(Debug, Params)]
pub struct VoiceParams {
    #[nested(id_prefix = "amp", group = "Amp")]
    amp: Arc<AdsrParams>,

    #[nested(id_prefix = "filter", group = "Filter")]
    filter: Arc<AdsrParams>,

    #[id = "fhz"]
    fhz: FloatParam,

    #[id = "q"]
    q: FloatParam,

    #[id = "fmod"]
    fmod: FloatParam,

    #[id = "drive"]
    drive: FloatParam,
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            amp: Arc::new(AdsrParams::default()),
            filter: Arc::new(AdsrParams::default()),
            fhz: FloatParam::new(
                "Filter Cutoff",
                300.,
                FloatRange::Skewed {
                    min: 20.,
                    max: 20e3,
                    factor: FloatRange::skew_factor(-2.),
                },
            )
            .with_string_to_value(formatters::s2v_f32_hz_then_khz())
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(2))
            .with_smoother(SmoothingStyle::Exponential(100.)),
            q: FloatParam::new(
                "Filter Q",
                0.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 16.,
                    factor: FloatRange::skew_factor(-2.),
                },
            )
            .with_smoother(SmoothingStyle::Linear(30.)),
            fmod: FloatParam::new(
                "Filter Modulation",
                3000.,
                FloatRange::Skewed {
                    min: 0.,
                    max: 20e3,
                    factor: FloatRange::skew_factor(-2.),
                },
            )
            .with_string_to_value(formatters::s2v_f32_hz_then_khz())
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(2))
            .with_smoother(SmoothingStyle::Exponential(100.)),
            drive: FloatParam::new(
                "Filter drive",
                0.,
                FloatRange::SymmetricalSkewed {
                    min: -36.,
                    max: 36.,
                    center: 0.,
                    factor: 2.,
                },
            )
            .with_unit("dB")
            .with_smoother(SmoothingStyle::Exponential(50.)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Voice {
    id: VoiceId,
    pub oscillator: Oscillator,
    velsqrt: f32,
    params: Arc<VoiceParams>,
    amp: Adsr,
    filter_adsr: Adsr,
    voice_gain: Option<(f32, Smoother<f32>)>,
    lpf: Ladder,
    // lpf: LP1,
}

impl PartialEq for Voice {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Voice {}

impl Voice {
    pub fn new(osc: Oscillator, id: VoiceId, velocity: f32, params: Arc<VoiceParams>) -> Self {
        let samplerate = osc.samplerate;
        Self {
            id,
            oscillator: osc,
            velsqrt: velocity.sqrt(),
            params: params.clone(),
            amp: Adsr::new(samplerate, params.amp.clone()),
            filter_adsr: Adsr::new(samplerate, params.filter.clone()),
            voice_gain: None,
            lpf: Ladder::new(samplerate, params.fhz.value(), params.q.value()),
            // lpf: LP1::new(samplerate, params.fhz.value()),
        }
    }

    pub fn release(&mut self) {
        self.amp.release();
    }

    pub fn done(&self) -> bool {
        !self.amp.active()
    }

    pub fn matches(&self, voice_id: Option<i32>, channel: u8, note: u8) -> bool {
        if let Some(id) = voice_id {
            self.id.is_id(id)
        } else {
            self.id.is_channel_note(channel, note)
        }
    }

    pub fn voice_id(&self) -> i32 {
        self.id.voice_id
    }

    #[cfg(never)]
    pub fn create_gain_smoother(
        &mut self,
        normalized_offset: f32,
        gain_smoother_gen: impl FnOnce() -> Smoother<f32>,
    ) -> &mut Smoother<f32> {
        let (_, smoother) = self
            .voice_gain
            .get_or_insert_with(|| (normalized_offset, gain_smoother_gen()));
        smoother
    }

    pub fn next_sample(&mut self) -> f32 {
        let gain = match self.voice_gain.as_ref() {
            Some((_, smoother)) => smoother.next(),
            None => 1.0,
        };
        let amp = self.amp.next() * gain * self.velsqrt;
        let drive = util::db_to_gain(self.params.drive.smoothed.next());
        self.lpf.set_fc(
            self.params.fhz.smoothed.next()
                + self.filter_adsr.next() * self.params.fmod.smoothed.next(),
        );
        self.lpf.set_resonance(self.params.q.smoothed.next());

        let osc = self.oscillator.sample();
        amp * self.lpf.process_sample(osc * drive) / drive
    }

    pub fn channel(&self) -> u8 {
        self.id.channel
    }

    pub fn note(&self) -> u8 {
        self.id.note
    }

    #[cfg(never)]
    pub fn update_gain(&mut self, normalized_value_gen: impl FnOnce(f32) -> f32) {
        if let Some((normalized_offset, smoother)) = self.voice_gain.as_mut() {
            smoother.set_target(
                self.oscillator.samplerate,
                normalized_value_gen(*normalized_offset),
            );
        }
    }

    pub fn id(&self) -> u64 {
        self.id.id
    }
}
