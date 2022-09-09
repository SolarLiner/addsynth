use crate::voice::{Voice, VoiceId};
use nih_plug::prelude::*;
use oscillator::Oscillator;
use rand::Rng;
use rand_pcg::Pcg32;
use std::sync::Arc;

mod phasor;
mod voice;
mod oscillator;

/// The number of simultaneous voices for this synth.
const NUM_VOICES: u32 = 16;
/// The maximum size of an audio block. We'll split up the audio in blocks and render smoothed
/// values to buffers since these values may need to be reused for multiple voices.
const MAX_BLOCK_SIZE: usize = 64;

// Polyphonic modulation works by assigning integer IDs to parameters. Pattern matching on these in
// `PolyModulation` and `MonoAutomation` events makes it possible to easily link these events to the
// correct parameter.
const GAIN_POLY_MOD_ID: u32 = 0;

/// A simple polyphonic synthesizer with support for CLAP's polyphonic modulation. See
/// `NoteEvent::PolyModulation` for another source of information on how to use this.
struct Addsynth {
    params: Arc<AddsynthParams>,

    /// A pseudo-random number generator. This will always be reseeded with the same seed when the
    /// synth is reset. That way the output is deterministic when rendering multiple times.
    prng: Pcg32,
    /// The synth's voices. Inactive voices will be set to `None` values.
    voices: [Option<Voice>; NUM_VOICES as usize],
}

impl Addsynth {
    fn create_voice(
        &mut self,
        ctx: &mut impl ProcessContext,
        sample_offset: u32,
        id: VoiceId,
        velocity: f32,
    ) -> &mut Voice {
        let samplerate = ctx.transport().sample_rate;
        let voice = Voice::new(
            Oscillator::triangle(samplerate, util::midi_note_to_freq(id.note)),
            id,
            velocity,
            self.params.amp_attack_ms.value(),
        );
        match self.voices.iter().position(|v| v.is_none()) {
            Some(free_voice_id) => {
                self.voices[free_voice_id] = Some(voice);
                return self.voices[free_voice_id].as_mut().unwrap();
            }
            None => {
                let oldest = unsafe {
                    self.voices
                        .iter_mut()
                        .min_by_key(|voice| voice.as_ref().unwrap_unchecked().id())
                        .unwrap_unchecked()
                };
                {
                    let oldest = oldest.as_ref().unwrap();
                    ctx.send_event(NoteEvent::VoiceTerminated {
                        timing: sample_offset,
                        voice_id: Some(oldest.voice_id()),
                        channel: oldest.channel(),
                        note: oldest.note(),
                    });
                }
                *oldest = Some(voice);
                return oldest.as_mut().unwrap();
            }
        }
    }
}

#[derive(Params)]
struct AddsynthParams {
    /// A voice's gain. This can be polyphonically modulated.
    #[id = "gain"]
    gain: FloatParam,
    /// The amplitude envelope attack time. This is the same for every voice.
    #[id = "amp_atk"]
    amp_attack_ms: FloatParam,
    /// The amplitude envelope release time. This is the same for every voice.
    #[id = "amp_rel"]
    amp_release_ms: FloatParam,
}

impl Default for Addsynth {
    fn default() -> Self {
        Self {
            params: Arc::new(AddsynthParams::default()),

            prng: Pcg32::new(420, 1337),
            // `[None; N]` requires the `Some(T)` to be `Copy`able
            voices: [0; NUM_VOICES as usize].map(|_| None),
        }
    }
}

impl Default for AddsynthParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(-12.0),
                // Because we're representing gain as decibels the range is already logarithmic
                FloatRange::Linear {
                    min: util::db_to_gain(-36.0),
                    max: util::db_to_gain(0.0),
                },
            )
            // This enables polyphonic mdoulation for this parameter by representing all related
            // events with this ID. After enabling this, the plugin **must** start sending
            // `VoiceTerminated` events to the host whenever a voice has ended.
            .with_poly_modulation_id(GAIN_POLY_MOD_ID)
            .with_smoother(SmoothingStyle::Logarithmic(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            amp_attack_ms: FloatParam::new(
                "Attack",
                200.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            // These parameters are global (and they cannot be changed once the voice has started).
            // They also don't need any smoothing themselves because they affect smoothing
            // coefficients.
            .with_step_size(0.1)
            .with_unit(" ms"),
            amp_release_ms: FloatParam::new(
                "Release",
                100.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 2000.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_step_size(0.1)
            .with_unit(" ms"),
        }
    }
}

impl Plugin for Addsynth {
    const NAME: &'static str = "Addsynth";
    const VENDOR: &'static str = "SolarLiner";
    const URL: &'static str = "https://youtu.be/dQw4w9WgXcQ";
    const EMAIL: &'static str = "solarliner@gmail.com";

    const VERSION: &'static str = "0.0.1";

    const DEFAULT_INPUT_CHANNELS: u32 = 0;
    const DEFAULT_OUTPUT_CHANNELS: u32 = 2;

    // We won't need any MIDI CCs here, we just want notes and polyphonic modulation
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    // If the synth as a variable number of voices, you will need to call
    // `context.set_current_voice_capacity()` in `initialize()` and in `process()` (when the
    // capacity changes) to inform the host about this.
    fn reset(&mut self) {
        // This ensures the output is at least somewhat deterministic when rendering to audio
        self.prng = Pcg32::new(420, 1337);

        self.voices.fill(None);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext,
    ) -> ProcessStatus {
        // NIH-plug has a block-splitting adapter for `Buffer`. While this works great for effect
        // plugins, for polyphonic synths the block size should be `min(MAX_BLOCK_SIZE,
        // num_remaining_samples, next_event_idx - block_start_idx)`. Because blocks also need to be
        // split on note events, it's easier to work with raw audio here and to do the splitting by
        // hand.
        let num_samples = buffer.len();
        let sample_rate = context.transport().sample_rate;
        let output = buffer.as_slice();

        let mut next_event = context.next_event();
        let mut block_start: usize = 0;
        let mut block_end: usize = MAX_BLOCK_SIZE.min(num_samples);
        while block_start < num_samples {
            // First of all, handle all note events that happen at the start of the block, and cut
            // the block short if another event happens before the end of it. To handle polyphonic
            // modulation for new notes properly, we'll keep track of the next internal note index
            // at the block's start. If we receive polyphonic modulation that matches a voice that
            // has an internal note ID that's great than or equal to this one, then we should start
            // the note's smoother at the new value instead of fading in from the global value.
            let next_id = VoiceId::next_id();
            'events: loop {
                match next_event {
                    // If the event happens now, then we'll keep processing events
                    Some(event) if (event.timing() as usize) <= block_start => {
                        // This synth doesn't support any of the polyphonic expression events. A
                        // real synth plugin however will want to support those.
                        match event {
                            NoteEvent::NoteOn {
                                timing,
                                voice_id,
                                channel,
                                note,
                                velocity,
                            } => {
                                let initial_phase = self.prng.gen();
                                let voice = self.create_voice(
                                    context,
                                    timing,
                                    VoiceId::new(voice_id, channel, note),
                                    velocity,
                                );
                                voice.oscillator.set_phase(initial_phase);
                            }
                            NoteEvent::NoteOff {
                                timing: _,
                                voice_id,
                                channel,
                                note,
                                velocity: _,
                            } => self.start_release_for_voices(voice_id, channel, note),
                            NoteEvent::Choke {
                                timing,
                                voice_id,
                                channel,
                                note,
                            } => {
                                self.choke_voices(context, timing, voice_id, channel, note);
                            }
                            NoteEvent::PolyModulation {
                                timing: _,
                                voice_id,
                                poly_modulation_id,
                                normalized_offset,
                            } => {
                                // Polyphonic modulation events are matched to voices using the
                                // voice ID, and to parameters using the poly modulation ID. The
                                // host will probably send a modulation event every N samples. This
                                // will happen before the voice is active, and of course also after
                                // it has been terminated (because the host doesn't know that it
                                // will be). Because of that, we won't print any assertion failures
                                // when we can't find the voice index here.
                                if let Some(voice_idx) = self.get_voice_idx(voice_id) {
                                    let voice = self.voices[voice_idx].as_mut().unwrap();

                                    match poly_modulation_id {
                                        GAIN_POLY_MOD_ID => {
                                            // This should either create a smoother for this
                                            // modulated parameter or update the existing one.
                                            // Notice how this uses the parameter's unmodulated
                                            // normalized value in combination with the normalized
                                            // offset to create the target plain value
                                            let target_plain_value = self
                                                .params
                                                .gain
                                                .preview_modulated(normalized_offset);
                                            let vid = voice.id();
                                            let smoother = voice
                                                .create_gain_smoother(normalized_offset, || {
                                                    self.params.gain.smoothed.clone()
                                                });

                                            // If this `PolyModulation` events happens on the
                                            // same sample as a voice's `NoteOn` event, then it
                                            // should immediately use the modulated value
                                            // instead of slowly fading in
                                            if vid >= next_id {
                                                smoother.reset(target_plain_value);
                                            } else {
                                                smoother
                                                    .set_target(sample_rate, target_plain_value);
                                            }
                                        }
                                        n => nih_debug_assert_failure!(
                                            "Polyphonic modulation sent for unknown poly \
                                             modulation ID {}",
                                            n
                                        ),
                                    }
                                }
                            }
                            NoteEvent::MonoAutomation {
                                timing: _,
                                poly_modulation_id,
                                normalized_value,
                            } => {
                                // Modulation always acts as an offset to the parameter's current
                                // automated value. So if the host sends a new automation value for
                                // a modulated parameter, the modulated values/smoothing targets
                                // need to be updated for all polyphonically modulated voices.
                                for voice in self.voices.iter_mut().filter_map(|v| v.as_mut()) {
                                    match poly_modulation_id {
                                        GAIN_POLY_MOD_ID => {
                                            voice.update_gain(|off| {
                                                self.params
                                                    .gain
                                                    .preview_plain(normalized_value + off)
                                            });
                                        }
                                        n => nih_debug_assert_failure!(
                                            "Automation event sent for unknown poly modulation ID \
                                             {}",
                                            n
                                        ),
                                    }
                                }
                            }
                            _ => (),
                        };

                        next_event = context.next_event();
                    }
                    // If the event happens before the end of the block, then the block should be cut
                    // short so the next block starts at the event
                    Some(event) if (event.timing() as usize) < block_end => {
                        block_end = event.timing() as usize;
                        break 'events;
                    }
                    _ => break 'events,
                }
            }

            // We'll start with silence, and then add the output from the active voices
            output[0][block_start..block_end].fill(0.0);
            output[1][block_start..block_end].fill(0.0);

            // These are the smoothed global parameter values. These are used for voices that do not
            // have polyphonic modulation applied to them. With a plugin as simple as this it would
            // be possible to avoid this completely by simply always copying the smoother into the
            // voice's struct, but that may not be realistic when the plugin has hundreds of
            // parameters. The `voice_*` arrays are scratch arrays that an individual voice can use.
            let block_len = block_end - block_start;
            let mut gain = [0.0; MAX_BLOCK_SIZE];
            self.params.gain.smoothed.next_block(&mut gain, block_len);

            // TODO: Amp envelope
            // TODO: Some form of band limiting
            // TODO: Filter
            for voice in self.voices.iter_mut().filter_map(|v| v.as_mut()) {
                for (value_idx, sample_idx) in (block_start..block_end).enumerate() {
                    let sample = voice.next_sample(gain[value_idx]);

                    output[0][sample_idx] += sample;
                    output[1][sample_idx] += sample;
                }
            }

            // Terminate voices whose release period has fully ended. This could be done as part of
            // the previous loop but this is simpler.
            for voice in self.voices.iter_mut() {
                match voice {
                    Some(v) if v.done() => {
                        // This event is very important, as it allows the host to manage its own modulation
                        // voices
                        context.send_event(NoteEvent::VoiceTerminated {
                            timing: block_end as u32,
                            voice_id: Some(v.voice_id()),
                            channel: v.channel(),
                            note: v.note(),
                        });
                        *voice = None;
                    }
                    _ => (),
                }
            }

            // And then just keep processing blocks until we've run out of buffer to fill
            block_start = block_end;
            block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);
        }

        ProcessStatus::Normal
    }
}

impl Addsynth {
    /// Get the index of a voice by its voice ID, if the voice exists. This does not immediately
    /// reutnr a reference to the voice to avoid lifetime issues.
    fn get_voice_idx(&mut self, voice_id: i32) -> Option<usize> {
        self.voices
            .iter_mut()
            .position(|voice| matches!(voice, Some(voice) if voice.voice_id() == voice_id))
    }

    /// Start the release process for one or more voice by changing their amplitude envelope. If
    /// `voice_id` is not provided, then this will terminate all matching voices.
    fn start_release_for_voices(&mut self, voice_id: Option<i32>, channel: u8, note: u8) {
        for voice in self
            .voices
            .iter_mut()
            .filter_map(|v| v.as_mut())
            .filter(|v| v.matches(voice_id, channel, note))
        {
            voice.release(self.params.amp_release_ms.value());
        }
    }

    /// Immediately terminate one or more voice, removing it from the pool and informing the host
    /// that the voice has ended. If `voice_id` is not provided, then this will terminate all
    /// matching voices.
    fn choke_voices(
        &mut self,
        context: &mut impl ProcessContext,
        sample_offset: u32,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
    ) {
        for voice in self.voices.iter_mut() {
            match voice {
                Some(v) if v.matches(voice_id, channel, note) => {
                    context.send_event(NoteEvent::VoiceTerminated {
                        timing: sample_offset,
                        // Notice how we always send the terminated voice ID here
                        voice_id: Some(v.voice_id()),
                        channel,
                        note,
                    });
                    *voice = None;

                    if voice_id.is_some() {
                        return;
                    }
                }
                _ => (),
            }
        }
    }
}

impl ClapPlugin for Addsynth {
    const CLAP_ID: &'static str = "com.github.solarliner.Addsynth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Additive synth");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for Addsynth {
    const VST3_CLASS_ID: [u8; 16] = *b"solaraddsynthvst";

    // And don't forget to change these categories, see the docstring on `VST3_CATEGORIES` for more
    // information
    const VST3_CATEGORIES: &'static str = "Instrument|Synth";
}

nih_export_clap!(Addsynth);
nih_export_vst3!(Addsynth);
