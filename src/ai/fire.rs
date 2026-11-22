//! The two controls that shoot without looking: `AlwaysShoot` and
//! `ShootPattern`.
//!
//! C# origin: `AiControlFactory` (`AlwaysShoot`: an `AiSimpleControl` whose
//! update is `Fire = Shoot; WeaponAxisControl = DirectionVector`) and
//! `PeriodicFireControl` (`ShootPattern`), in `AiSimpleControl.cs`.
//!
//! - `AlwaysShoot` pulls the trigger every step, the gun pointing where the
//!   car does; the weapon's own rhythm (the Minigun's burst and reload,
//!   the Railgun's reload) decides what comes out.
//! - `ShootPattern` picks one of four patterns when the car appears
//!   ([`PATTERNS`]: `0` is a quarter second of silence, `1` a quarter
//!   second of fire, spaces ignored) and plays it round and round: the
//!   trigger is held, the gun pointing where the car does, in the fire
//!   phases, and let go in the silent ones (the aim stays where it was). A
//!   pattern that starts with `1` begins with a silence of no length, as
//!   in C#'s way of cutting the string into phases.
//!
//! Neither looks for a target, so neither needs the world.
//!
//! - `PeriodicFireControl`'s pre-fire: in the tail of a silence the trigger
//!   is pulled early for a gun with a `ChargeTime` (only `LaserWithDelay`'s
//!   decorator has one) that cannot shoot (one that is charging), when what
//!   is left of the silence is no more than the charge. As C# had it, a
//!   gun not charging can shoot, so the pre-fire only goes on with a charge
//!   begun in a fire phase; it never begins one.
//!
//! Not ported: the pattern's empty-string default (`1 s` silence, `0.25 s` fire: the constructor
//! with no pattern is never called with one).

use bevy::prelude::Component;
use rand::Rng;

/// `PeriodicFireControl._patterns`.
pub const PATTERNS: [&str; 4] = ["0000 1 0 1 0 1111", "1111 0 1 0000", "0000 1111 00 1111", "0000 1 00 1"];

/// `PeriodicFireControl`'s `shortTime`: one character, seconds.
pub const BEAT: f32 = 0.25;

/// `AlwaysShoot`: the trigger down all the time.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct AlwaysShoot;

/// `PeriodicFireControl`: a pattern of silences and fire, in seconds.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ShootPattern {
    /// Even places are silences, odd ones fire.
    timing: Vec<f32>,
    index: usize,
    /// Seconds in the present phase (`_previousTime`).
    since: f32,
}

impl ShootPattern {
    /// `PeriodicFireControl(world, team, pattern)`: the phase lengths of a
    /// pattern.
    pub fn new(pattern: &str) -> ShootPattern {
        let mut timing = Vec::new();
        let mut current = 0.0;
        let mut index = 0;
        for c in pattern.chars().filter(|c| matches!(c, '0' | '1')) {
            let expected = if index % 2 == 0 { '0' } else { '1' };
            if c == expected {
                current += BEAT;
            } else {
                timing.push(current);
                index += 1;
                current = BEAT;
            }
        }
        timing.push(current);
        ShootPattern {
            timing,
            index: 0,
            since: 0.0,
        }
    }

    /// One of the four, at random (`GetRandomPattern`).
    pub fn random(rng: &mut impl Rng) -> ShootPattern {
        ShootPattern::new(PATTERNS[rng.random_range(0..PATTERNS.len())])
    }

    /// The phase lengths, seconds.
    pub fn timing(&self) -> &[f32] {
        &self.timing
    }

    /// Seconds left of the present phase (`_timing[_currentIndex]` less the
    /// time since `_previousTime`).
    pub fn remaining(&self) -> f32 {
        self.timing[self.index] - self.since
    }

    /// `UpdateControl`: time passes (one phase at most a step); whether the
    /// trigger is held now.
    pub fn step(&mut self, dt: f32) -> bool {
        self.since += dt;
        // `!CheckSpan`: the phase is over when its time is up (a millionth
        // of a second spare for the sums, as the weapons' timers).
        if crate::weapon::model::due(self.timing[self.index] - self.since) {
            self.index = (self.index + 1) % self.timing.len();
            self.since = 0.0;
        }
        self.index % 2 == 1
    }
}
