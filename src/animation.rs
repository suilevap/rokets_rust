//! Animation as plain data: waves over game time and colour keyframes.
//!
//! The C# game drove its small animations from one shared clock,
//! `PhaseHelper`, which paused and slowed down with the game: a few phases
//! (a 1 s sawtooth, a 1 s triangle, a 6.28 s sawtooth…) that blinking pads,
//! pulsing highlights, spinning gears, buttons and hints all read, so they
//! stay in step. One-shot effects (a flash after use) were timers whose
//! remaining part blended two colours (`PeriodicUpdate.RemainPart`).
//!
//! Here a [`Wave`] gives the phase at a moment of game time, a
//! [`ColorTrack`] gives the colour at a phase, and a [`Pulse`] gives a
//! one-shot blend. Components in `render/animate.rs` apply them to sprites.
//! No Bevy app: unit-testable.

/// An RGBA colour, sRGB 0..1.
pub type Rgba = [f32; 4];

/// `Color.Yellow`, `Color.Gray`, `Color.White` in XNA.
pub const YELLOW: Rgba = [1.0, 1.0, 0.0, 1.0];
pub const GRAY: Rgba = [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0];
pub const WHITE: Rgba = [1.0; 4];

/// A repeating phase 0..1 over game time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    /// Rises from 0 to 1, then starts again (`Sawtooth_Period…`).
    Sawtooth { period: f32 },
    /// Rises from 0 to 1 and falls back over one period (`Triangle_Period…`).
    Triangle { period: f32 },
}

impl Wave {
    /// The phase at `t` seconds of game time.
    pub fn phase(self, t: f32) -> f32 {
        match self {
            Wave::Sawtooth { period } => (t / period).rem_euclid(1.0),
            Wave::Triangle { period } => {
                let p = (t / period).rem_euclid(1.0) * 2.0;
                if p > 1.0 { 2.0 - p } else { p }
            }
        }
    }
}

pub fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// Colour keyframes over a phase 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorTrack {
    /// `(phase, colour)`, sorted by phase; the first should be at 0.
    pub keys: Vec<(f32, Rgba)>,
    /// Hold each key until the next (`true`), or blend between them.
    pub step: bool,
}

impl ColorTrack {
    /// Holds each colour until the next key.
    pub fn steps(keys: &[(f32, Rgba)]) -> Self {
        ColorTrack {
            keys: keys.to_vec(),
            step: true,
        }
    }

    pub fn sample(&self, phase: f32) -> Rgba {
        let Some(&(_, first)) = self.keys.first() else {
            return WHITE;
        };
        let i = self.keys.iter().rposition(|&(at, _)| at <= phase);
        let Some(i) = i else {
            return first;
        };
        let (at, color) = self.keys[i];
        match self.keys.get(i + 1) {
            Some(&(next_at, next)) if !self.step && next_at > at => {
                lerp(color, next, (phase - at) / (next_at - at))
            }
            _ => color,
        }
    }
}

/// A one-shot blend from `start` to `end` over `duration` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    pub start: Rgba,
    pub end: Rgba,
    pub duration: f32,
}

impl Pulse {
    /// The colour `elapsed` seconds in, or `None` once it is over.
    pub fn sample(&self, elapsed: f32) -> Option<Rgba> {
        (elapsed < self.duration).then(|| lerp(self.start, self.end, elapsed / self.duration))
    }
}
