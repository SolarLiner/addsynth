use nih_plug::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::{phasor::Phasor, oscillator::Oscillator};

static NEXT_VOICE_ID: AtomicU64 = AtomicU64::new(0);

/// Compute a voice ID in case the host doesn't provide them. Polyphonic modulation will not work in
/// this case, but playing notes will.
const fn compute_fallback_voice_id(note: u8, channel: u8) -> i32 {
    note as i32 | ((channel as i32) << 16)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn note_phasor(&self, samplerate: f32) -> Phasor {
        Phasor::new(samplerate, util::midi_note_to_freq(self.note))
    }

    pub fn next_id() -> u64 {
        NEXT_VOICE_ID.load(Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct Voice {
    id: VoiceId,
    pub oscillator: Oscillator,
    velsqrt: f32,
    releasing: bool,
    amp_envelope: Smoother<f32>,
    voice_gain: Option<(f32, Smoother<f32>)>,
}

impl Voice {
    pub fn new(osc: Oscillator, id: VoiceId, velocity: f32, a: f32) -> Self {
        let samplerate = osc.samplerate();
        let amp_envelope = Smoother::new(SmoothingStyle::Logarithmic(a));
        amp_envelope.reset(1e-4);
        amp_envelope.set_target(samplerate, 1.0);

        let phase = id.note_phasor(samplerate);

        Self {
            id,
            oscillator: osc,
            velsqrt: velocity.sqrt(),
            releasing: false,
            amp_envelope,
            voice_gain: None,
        }
    }

    pub fn release(&mut self, r: f32) {
        self.amp_envelope = Smoother::new(SmoothingStyle::Exponential(r));
        self.amp_envelope.reset(1.0);
        self.amp_envelope.set_target(self.oscillator.samplerate(), 0.0);
        self.releasing = true;
    }

    pub fn done(&self) -> bool {
        self.releasing && self.amp_envelope.previous_value() == 0.0
    }

    pub fn matches(&self, voice_id: Option<i32>, channel: u8, note: u8) -> bool {
        let candidate = voice_id.unwrap_or_else(|| compute_fallback_voice_id(channel, note));
        self.id.voice_id == candidate
    }

    pub fn voice_id(&self) -> i32 {
        self.id.voice_id
    }

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

    pub fn next_sample(&mut self, global_gain: f32) -> f32 {
        let gain = match self.voice_gain.as_ref() {
            Some((_, smoother)) => smoother.next(),
            None => 1.0,
        };
        let amp = self.amp_envelope.next() * gain * global_gain * self.velsqrt;
        (self.oscillator.sample() * amp).tanh()
    }

    pub fn channel(&self) -> u8 {
        self.id.channel
    }

    pub fn note(&self) -> u8 {
        self.id.note
    }

    pub fn update_gain(&mut self, normalized_value_gen: impl FnOnce(f32) -> f32) {
        if let Some((normalized_offset, smoother)) = self.voice_gain.as_mut() {
            smoother.set_target(self.oscillator.samplerate(), normalized_value_gen(*normalized_offset));
        }
    }

    pub fn id(&self) -> u64 {
        self.id.id
    }
}
