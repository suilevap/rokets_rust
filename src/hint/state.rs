//! What a hint does over time, with no Bevy in it: the fade in and out,
//! the pulse, the countdown, the frames of an animation and the checks of
//! the player's place. Plain data stepped by the elapsed time and by
//! numbers the caller draws (`rolls`, the phase), so a test can drive it.
//!
//! C# origin: the update half of `Entities/HintTile.cs` (and its nested
//! `Timer`), the frame counting of `animation/Animation.cs`.

use crate::animation::Wave;

/// `HintTile._showAfterTime`: how long a hint takes to flicker in, and to
/// fade out, seconds.
pub const FADE_SECS: f32 = 1.0;
/// `new PeriodicUpdate(1.0f)`: how often the player's place is checked,
/// seconds.
pub const CHECK_EVERY: f32 = 1.0;
/// `Triangle_Period500`: the period of the pulse and of the blink, seconds
/// (the phase rises over half of it and falls over the rest).
pub const PULSE_PERIOD: f32 = 1.0;
/// `HintTile`'s `0.2f * pulseAmount`: how far one unit of `PulseAmount`
/// swings the size.
const PULSE_SWING: f32 = 0.2;
/// The blink of a plain hint: it never goes darker than this share.
const BLINK_FLOOR: f32 = 0.3;
/// The animation's `frameTime` (`CreateAnimatedView(…, 0.5f, …)`), seconds.
pub const FRAME_SECS: f32 = 0.5;

/// `PhaseHelper.Triangle_Period500`: 0 to 1 and back over a second of game
/// time.
pub fn triangle(game_secs: f32) -> f32 {
    Wave::Triangle { period: PULSE_PERIOD }.phase(game_secs)
}

/// `HintTile._timer` (`HintTile.Timer`): a countdown that stops at 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Countdown {
    pub set: f32,
    pub remains: f32,
}

impl Countdown {
    /// The text it shows (`ToString("##0.0")`): one decimal.
    pub fn text(&self) -> String {
        format!("{:.1}", self.remains)
    }
}

/// One hint's life: off, flickering in, shown, fading out.
#[derive(Clone, Debug, PartialEq)]
pub struct HintState {
    /// `Entity.IsActive`. Only an active hint is updated and drawn.
    pub active: bool,
    /// `Entity.Visible`: the flicker toggles it.
    pub visible: bool,
    /// `_isDeactivating`: fading out; it goes off when it is done.
    pub deactivating: bool,
    /// `_activeTime`: seconds into the fade, up to [`FADE_SECS`].
    active_time: f32,
    /// `_tmpColor` as a share of `MainColor` (white): the opacity.
    pub opacity: f32,
    /// `_tmpScale / _scaleDisplay`.
    pub size: f32,
    /// `0.2 * pulseAmount`.
    pulse: f32,
    /// Shows a text, so it does not blink.
    has_text: bool,
    pub countdown: Option<Countdown>,
    /// `PeriodicUpdate._invalidatedTime`: when the player's place is next
    /// looked at, game seconds.
    next_check: f32,
}

impl HintState {
    /// `new HintTile(…, timer, text, pulseAmount)`: off and hidden.
    pub fn new(timer: f32, has_text: bool, pulse_amount: f32) -> Self {
        HintState {
            active: false,
            visible: false,
            deactivating: false,
            active_time: 0.0,
            // `_tmpColor` and `_tmpScale` start at zero; nothing reads them
            // before the first update sets them.
            opacity: 0.0,
            size: 0.0,
            pulse: PULSE_SWING * pulse_amount,
            has_text,
            countdown: (timer > 0.0).then_some(Countdown { set: timer, remains: 0.0 }),
            next_check: 0.0,
        }
    }

    /// `HintTile.Activate`: on, at once visible, the countdown starts over.
    pub fn activate(&mut self) {
        self.active = true;
        self.visible = true;
        self.deactivating = false;
        if let Some(c) = &mut self.countdown {
            c.remains = c.set;
        }
    }

    /// `HintTile.Deactivate`: start fading out. It stays active until the
    /// fade is done, and a hint that is not active ignores it (it only
    /// starts over at the next [`HintState::activate`]).
    pub fn deactivate(&mut self) {
        self.deactivating = true;
    }

    /// `PeriodicUpdate.NeedUpdate(gameTime)` at `now` game seconds: due
    /// once the time is past the mark, which then moves a period on. The
    /// first check is at once.
    pub fn check_due(&mut self, now: f32) -> bool {
        let due = now > self.next_check;
        if due {
            self.next_check = now + CHECK_EVERY;
        }
        due
    }

    /// `HintTile.CheckArea`'s decision: switch on when the player is in the
    /// area and the hint is off, start fading when the player left and it
    /// is on. A hint that is still fading out when the player returns is
    /// left to finish.
    pub fn player_is_in_area(&mut self, in_area: bool) {
        if in_area && !self.active {
            self.activate();
        } else if !in_area && self.active {
            self.deactivate();
        }
    }

    /// `HintTile.Update` after the area check: `dt` game seconds, `phase`
    /// the [`triangle`] of game time, `chance(p)` true with probability `p`
    /// (`RandomHelper.RandomEvent`).
    pub fn update(&mut self, dt: f32, phase: f32, mut chance: impl FnMut(f32) -> bool) {
        if !self.active {
            return;
        }
        if !self.deactivating {
            if let Some(c) = &mut self.countdown {
                c.remains = (c.remains - dt).max(0.0);
            }
            if self.active_time < FADE_SECS {
                self.active_time += dt;
                // Flicker in: it comes on more and more often and goes
                // off less and less, a brighter step each time.
                if chance(0.3) {
                    let share = self.active_time / FADE_SECS;
                    if !self.visible || chance(1.0 - share) {
                        self.visible = !self.visible;
                        self.opacity = (share * 0.8 + 0.2).clamp(0.0, 1.0);
                    }
                }
            } else {
                self.active_time = FADE_SECS;
                self.visible = true;
                self.opacity = if self.has_text { 1.0 } else { BLINK_FLOOR + (1.0 - BLINK_FLOOR) * phase };
            }
        } else {
            self.active_time -= dt;
            self.opacity = (self.active_time / FADE_SECS).clamp(0.0, 1.0);
            if self.active_time <= 0.0 {
                self.active_time = 0.0;
                self.opacity = 1.0;
                self.size = 1.0;
                self.visible = false;
                self.active = false;
            }
        }
        self.size = 1.0 - self.pulse + self.pulse * phase;
    }
}

/// `Animation`: a row of frames shown in turn, looping.
#[derive(Clone, Debug, PartialEq)]
pub struct Frames {
    /// The frame (an index into the row) each step shows.
    sequence: Vec<usize>,
    /// `FrameIndex`: where in the sequence.
    at: usize,
    /// Seconds the current frame has been shown.
    time: f32,
}

impl Frames {
    /// All `count` frames in order, or the given order.
    pub fn new(count: usize, sequence: Option<&[i64]>) -> Self {
        let sequence = match sequence {
            Some(s) => s.iter().map(|&i| i.max(0) as usize).collect(),
            None => (0..count).collect(),
        };
        Frames { sequence, at: 0, time: 0.0 }
    }

    /// Time passes while the hint is drawn: past a frame's time it moves
    /// on, by as many as fit (`Animation.Draw`).
    pub fn advance(&mut self, dt: f32) {
        if self.sequence.is_empty() {
            return;
        }
        self.time += dt;
        if self.time > FRAME_SECS {
            let skip = (self.time / FRAME_SECS) as usize;
            self.time -= skip as f32 * FRAME_SECS;
            self.at = (self.at + skip) % self.sequence.len();
        }
    }

    /// The frame to draw, in the row.
    pub fn current(&self) -> usize {
        self.sequence.get(self.at).copied().unwrap_or(0)
    }
}
