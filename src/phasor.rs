#[derive(Debug, Clone, Copy)]
pub struct Phasor {
    pub hz: f32,
    pub samplerate: f32,
    pub phase: f32,
}

impl Phasor {
    pub fn new(samplerate: f32, hz: f32) -> Self {
        Self {
            hz,
            samplerate,
            phase: 0.0,
        }
    }

    pub fn step(&self) -> f32 {
        self.hz / self.samplerate
    }

    pub fn inc(&mut self, amt: u32) -> f32 {
        self.set_phase(self.phase + amt as f32 * self.step())
    }

    pub fn set_phase(&mut self, phase: f32) -> f32 {
        if phase < 0.0 {
            self.set_phase(phase + 1.0)
        } else {
            self.phase = phase % 1.0;
            self.phase
        }
    }
}
