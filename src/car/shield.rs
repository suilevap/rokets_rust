//! A car's shield (`ShieldComponent`): points that soak up damage before
//! the health does, and come back after a quiet spell.
//!
//! - Damage first takes the shield's whole points; what the shield cannot
//!   cover goes on to the health. The shield counts in whole points: what
//!   is left after a blow is rounded down (`(byte)`), what has grown back
//!   is a fraction until then.
//! - Every blow, even on a shield that is down, restarts a 4 s quiet
//!   period ([`QUIET`]); after it the shield grows back 4 points a second
//!   until it is whole.
//! - A car with a shield shows it instead of its health on its bar while
//!   any of it is left (`DrawHealth`).
//! - An `Immortal` shield (`SetImmortal`, set by `actions/immortal`) takes
//!   a blow whole and loses nothing, as long as it has a point left: a
//!   shield that is down passes everything on, immortal or not
//!   (`Spend`'s `Value > 0` guard). A blow still restarts the quiet period.
//!   The car shows no health bar then (`TakeDamage`, `health_bar`).
//!
//! Not ported: the bubble drawn as it comes back, in full while the shield
//! is immortal, and its burst when it breaks or stops being immortal
//! (`Draw`, `CreateShieldDestroyEffect`, the `Immortal` setter), and the
//! shield's hit sound when it takes a whole blow (`TakeDamage` playing
//! `ShieldDamage` when nothing passes on).

use avian2d::prelude::{Physics, PhysicsTime};
use bevy::prelude::*;

use crate::game::GameState;

/// `_rechargePeriod`: seconds without a blow before it grows back.
pub const QUIET: f32 = 4.0;
/// `_rechargeSpeedPerSec`.
pub const RECHARGE_PER_SEC: f32 = 4.0;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shield {
    value: f32,
    max: f32,
    /// Seconds since the last blow.
    quiet: f32,
    /// `Immortal`: blows do it no harm while it has points.
    immortal: bool,
}

impl Shield {
    /// A whole shield of `max` points (`ShieldComponent(int)`). It starts
    /// with its quiet period over (`PeriodicUpdate` begins available).
    pub fn new(max: u8) -> Shield {
        Shield {
            value: max as f32,
            max: max as f32,
            quiet: QUIET,
            immortal: false,
        }
    }

    /// `Immortal`.
    pub fn is_immortal(&self) -> bool {
        self.immortal
    }

    /// `Immortal`'s setter.
    pub fn set_immortal(&mut self, immortal: bool) {
        self.immortal = immortal;
    }

    /// `Value`: whole points left.
    pub fn points(&self) -> u8 {
        self.value as u8
    }

    /// `FloatValue`: the points with the part that has grown back.
    pub fn float_value(&self) -> f32 {
        self.value
    }

    /// `MaxValue`.
    pub fn max(&self) -> u8 {
        self.max as u8
    }

    /// `ValuePart`: the share left, 0..1 (a hair over right after it has
    /// grown back).
    pub fn part(&self) -> f32 {
        self.value / self.max
    }

    /// `Spend`: takes `damage` and returns what is left of it for the
    /// health. An immortal shield with points left takes it all and stays
    /// as it was.
    pub fn spend(&mut self, damage: i32) -> i32 {
        self.quiet = 0.0;
        if self.points() == 0 {
            return damage;
        }
        if self.immortal {
            return 0;
        }
        let left = self.points() as i32 - damage;
        if left < 0 {
            self.value = 0.0;
            -left
        } else {
            self.value = left as f32;
            0
        }
    }

    /// `Update`: after the quiet spell it grows back, until whole.
    pub fn step(&mut self, dt: f32) {
        self.quiet += dt;
        if self.points() < self.max() && self.quiet > QUIET {
            self.value += RECHARGE_PER_SEC * dt;
        }
    }
}

pub struct ShieldPlugin;

impl Plugin for ShieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, recharge.run_if(in_state(GameState::Playing)));
    }
}

fn recharge(time: Res<Time>, physics: Res<Time<Physics>>, mut shields: Query<&mut Shield>) {
    if physics.is_paused() {
        return;
    }
    for mut shield in &mut shields {
        shield.step(time.delta_secs());
    }
}
