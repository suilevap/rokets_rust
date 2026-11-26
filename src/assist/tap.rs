//! Taps: what a finger (or a mouse click) that went down and came up
//! again meant, and what a tap on the ground points at.
//!
//! Both are plain functions of numbers, tested in `tests/assist.rs`; the
//! devices that call them are in [`crate::controls`], the commands they
//! lead to in [`super`].

use bevy::prelude::*;

/// A press that lasts longer than this is not a tap, s.
pub const TAP_MAX_SECS: f32 = 0.25;
/// A press that moves further than this is a drag, in logical pixels at
/// the reference screen height ([`slop`] scales it).
pub const TAP_SLOP_PX: f32 = 12.0;
/// The screen height [`TAP_SLOP_PX`] is for (the C# game's 800 × 480).
const REFERENCE_HEIGHT: f32 = 480.0;
/// A tap on the ground picks the nearest enemy within this of the tap, world
/// units (a tile is one unit): 1.5 tiles, so a thumb need not hit the car.
pub const TAP_PICK_RADIUS: f32 = 1.5;

/// A tap on the player's own car counts within this of its middle at least,
/// world units, whatever the car's size.
pub const OWN_CAR_MIN: f32 = 1.2;
/// ... or within the car's radius plus this.
pub const OWN_CAR_MARGIN: f32 = 0.5;

/// How near the middle of a car of `radius` a tap is on that car.
pub fn own_car_reach(radius: f32) -> f32 {
    (radius + OWN_CAR_MARGIN).max(OWN_CAR_MIN)
}

/// What a press is, so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// Down for less than [`TAP_MAX_SECS`] and not moved: it may yet be
    /// either; nothing is driven.
    Pending,
    /// Up again in time, without having moved: a tap.
    Tap,
    /// Moved too far, or held too long: the stick (a press that is only
    /// held is a stick with nothing pushed, not a tap).
    Drag,
}

/// How far a press may move and still be a tap, logical pixels: the same
/// look on a big or a small screen, as the menu scales its rows (a phone's
/// screen height is near the reference; a tablet or a desktop window
/// scales up to twice).
pub fn slop(screen: Vec2) -> f32 {
    TAP_SLOP_PX * (screen.min_element() / REFERENCE_HEIGHT).clamp(1.0, 2.0)
}

/// What a press that went down at `origin` `held` seconds ago and is at
/// `now` is (`released`: it has just come up).
pub fn classify(origin: Vec2, now: Vec2, held: f32, released: bool, slop: f32) -> Gesture {
    if origin.distance(now) >= slop || held > TAP_MAX_SECS {
        Gesture::Drag
    } else if released {
        Gesture::Tap
    } else {
        Gesture::Pending
    }
}

/// The enemy a tap at `at` (on the ground, world) points at: the nearest of
/// `enemies` (entity and middle; the caller has left out allies and the
/// dead) within `radius` of it.
pub fn tap_target(enemies: impl IntoIterator<Item = (Entity, Vec2)>, at: Vec2, radius: f32) -> Option<Entity> {
    let mut best: Option<(f32, Entity)> = None;
    for (entity, middle) in enemies {
        let d = middle.distance_squared(at);
        if d <= radius * radius && best.is_none_or(|b| d < b.0) {
            best = Some((d, entity));
        }
    }
    best.map(|b| b.1)
}

/// What a tap on the ground points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// An enemy: lock it.
    Enemy(Entity),
    /// A thing the magnet could take.
    Thing(Entity),
    None,
}

/// What a tap at `at` points at: the nearest of the `enemies` and the
/// `things` (entity and middle each; the caller has left out what does not
/// count) within `radius`. Where an enemy and a thing are equally near, the
/// enemy: the gun first.
pub fn tap_pick(
    enemies: impl IntoIterator<Item = (Entity, Vec2)>,
    things: impl IntoIterator<Item = (Entity, Vec2)>,
    at: Vec2,
    radius: f32,
) -> Pick {
    let nearest = |items: &mut dyn Iterator<Item = (Entity, Vec2)>| {
        let mut best: Option<(f32, Entity)> = None;
        for (entity, middle) in items {
            let d = middle.distance_squared(at);
            if d <= radius * radius && best.is_none_or(|b| d < b.0) {
                best = Some((d, entity));
            }
        }
        best
    };
    match (nearest(&mut enemies.into_iter()), nearest(&mut things.into_iter())) {
        (Some(e), Some(t)) if t.0 < e.0 => Pick::Thing(t.1),
        (Some(e), _) => Pick::Enemy(e.1),
        (None, Some(t)) => Pick::Thing(t.1),
        (None, None) => Pick::None,
    }
}

/// A press being watched: where and when it went down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    pub origin: Vec2,
    /// Where it was last seen (a finger that is lifted is no longer read).
    pub last: Vec2,
    pub since: f32,
}

impl Press {
    pub fn new(at: Vec2, now: f32) -> Press {
        Press { origin: at, last: at, since: now }
    }

    /// The gesture at time `now`, the finger at `at` (`None` for a finger
    /// that is up: its last place counts).
    pub fn gesture(&mut self, at: Option<Vec2>, now: f32, slop: f32) -> Gesture {
        if let Some(at) = at {
            self.last = at;
        }
        classify(self.origin, self.last, now - self.since, at.is_none(), slop)
    }
}
