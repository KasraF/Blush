use nih_plug::prelude::*;
use std::sync::Arc;

struct Osc {
    params: Arc<OscParams>,
    sample_rate: f32,
    phase: f32,
    midi_note_id: u8,
    midi_note_freq: f32,
    midi_note_gain: Smoother<f32>,
}

#[derive(Enum, PartialEq)]
enum OscMode {
    #[name = "Sine Wave"]
    #[id = "sine"]
    Sine,
}

#[derive(Params)]
struct OscParams {
    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "freq"]
    pub freq: FloatParam,

    #[id = "mode"]
    pub mode: EnumParam<OscMode>,
}

impl Default for Osc {
    fn default() -> Self {
        Self {
            params: Arc::new(OscParams::default()),
            sample_rate: 1.0, // TODO ???
            phase: 0.0,
            midi_note_id: 0,
            midi_note_freq: 1.0,
            midi_note_gain: Smoother::new(SmoothingStyle::Linear(5.0)),
        }
    }
}

impl Default for OscParams {
    fn default() -> Self {
        let gain = FloatParam::new(
            "Gain",
            -10.0,
            FloatRange::Linear {
                min: -30.0,
                max: 0.0,
            },
        )
        .with_smoother(SmoothingStyle::Linear(3.0))
        .with_step_size(0.01)
        .with_unit(" dB");

        let freq: FloatParam = FloatParam::new(
            "Frequencey",
            420.0,
            FloatRange::Skewed {
                min: 1.0,
                max: 20_000.0,
                factor: FloatRange::skew_factor(-2.0),
            },
        )
        .with_smoother(SmoothingStyle::Linear(10.0))
        .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
        .with_string_to_value(formatters::s2v_f32_hz_then_khz());

        let mode = EnumParam::new("Mode", OscMode::Sine);

        Self { gain, freq, mode }
    }
}

impl Osc {
    fn calculate_sine(&mut self, frequency: f32) -> f32 {
        let phase_delta = frequency / self.sample_rate;
        let sine = (self.phase * std::f32::consts::TAU).sin();

        self.phase += phase_delta;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sine
    }
}

impl Plugin for Osc {
    const NAME: &'static str = "Blush Oscillator";
    const VENDOR: &'static str = "Weird Machine";
    const URL: &'static str = "https://weirdmachine.me";
    const EMAIL: &'static str = "kferdowsifard@gmail.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _bus_config: &BusConfig,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        // TODO (kas) This is... terribly inefficient to say the least.
        let tmp = Self::default();
        self.phase = tmp.phase;
        self.midi_note_freq = tmp.midi_note_freq;
        self.midi_note_gain = tmp.midi_note_gain;
        self.midi_note_id = tmp.midi_note_id;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for (sid, ch_samples) in buffer.iter_samples().enumerate() {
            let gain = self.params.gain.smoothed.next(); // .next() ?!

            let sine = {
                while let Some(event) = context.next_event() {
                    if event.timing() > sid as u32 {
                        break;
                    }

                    match event {
                        NoteEvent::NoteOn { note, velocity, .. } => {
                            self.midi_note_id = note;
                            self.midi_note_freq = util::midi_note_to_freq(note);
                            self.midi_note_gain.set_target(self.sample_rate, velocity);
                        }
                        NoteEvent::NoteOff { note, .. } if note == self.midi_note_id => {
                            self.midi_note_gain.set_target(self.sample_rate, 0.0)
                        }
                        NoteEvent::PolyPressure { note, pressure, .. }
                            if note == self.midi_note_id =>
                        {
                            self.midi_note_gain.set_target(self.sample_rate, pressure);
                        }
                        _ => (),
                    }
                }

                self.calculate_sine(self.midi_note_freq) * self.midi_note_gain.next()
            };

            for sample in ch_samples {
                // TODO (kas) using this gain adds a 1-sample delay to gain adjustment.
                // Why?
                *sample = sine * util::db_to_gain_fast(gain);
            }
        }
        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for Osc {
    const CLAP_ID: &'static str = "me.weirdmachine.blush";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A MIDI controlled Oscillator");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

nih_export_clap!(Osc);
