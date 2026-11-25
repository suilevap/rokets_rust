//! The magnet's rope (`Entities/Chain.cs`, `ElasticLinearVector2`): a few
//! points each kept about a segment's length from the next, relaxed from
//! the car out and from the far end back every step. Linked to cargo it
//! pulls by how far the free rope's end lags behind the cargo's point:
//! further than its seven segments it pulls the cargo in, harder the
//! further; nearer it pushes it out a little, so the cargo trails 3.5
//! units behind, like on a tow bar.

use bevy::math::Vec2;

/// `Chain`'s points: `CreateChain(…, ToSimUnits(128), 8)`.
pub const POINTS: usize = 8;
/// The rope's length at rest, world units (128 px).
pub const LENGTH: f32 = 128.0 / 32.0;
/// How far it stretches before it breaks (`MaxLength`).
pub const MAX_LENGTH: f32 = LENGTH * 3.0;
/// Force per unit the free end lags behind the cargo (`force *= 256`).
pub const PULL: f32 = 256.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Rope {
    /// Each point's place and where it is drawn to.
    current: Vec<Vec2>,
    destination: Vec<Vec2>,
    segment: f32,
}

/// `ElasticLinearVector2.Update`: `current` a segment from `destination`,
/// a quarter of the way there each step.
fn settle(current: Vec2, destination: Vec2, segment: f32) -> Vec2 {
    let d = current - destination;
    if d == Vec2::ZERO {
        destination
    } else {
        (destination + d.normalize() * segment).lerp(current, 0.25)
    }
}

impl Rope {
    pub fn new(at: Vec2) -> Rope {
        Rope {
            current: vec![at; POINTS],
            destination: vec![at; POINTS],
            segment: LENGTH / POINTS as f32,
        }
    }

    /// One step from `from` (the car) to `to` (the far end): the force on
    /// the cargo at `to`, as `Chain.Update` gave it (when linked).
    pub fn step(&mut self, from: Vec2, to: Vec2) -> Vec2 {
        let n = POINTS;
        let s = self.segment;
        self.destination[0] = from;
        self.current[0] = settle(self.current[0], from, s);
        for i in 1..n {
            self.destination[i] = self.current[i - 1];
            self.current[i] = settle(self.current[i], self.destination[i], s);
        }
        let force = (self.current[n - 1] - to) * PULL;
        // Back from the far end (`UpdateInv`).
        self.current[n - 1] = to;
        self.destination[n - 1] = settle(self.destination[n - 1], to, s);
        for i in (1..n - 1).rev() {
            self.current[i] = self.destination[i + 1];
            self.destination[i] = settle(self.destination[i], self.current[i], s);
        }
        self.destination[0] = from;
        self.current[n - 1] = to;
        force
    }

    /// The points to draw, car first.
    pub fn points(&self) -> &[Vec2] {
        &self.current
    }

    /// All points evenly spread on the line from `from` to `to`
    /// (`SetElementsPositionByLine`, as it meant: see `apply`'s "Differences").
    pub fn lay(&mut self, from: Vec2, to: Vec2) {
        for (i, p) in self.current.iter_mut().enumerate() {
            *p = from.lerp(to, i as f32 / (POINTS - 1) as f32);
        }
        self.destination.clone_from(&self.current);
    }

    /// The far end where it can reach: no further than [`MAX_LENGTH`]
    /// from the car (`SetLinkedToPos`).
    pub fn reach(from: Vec2, to: Vec2) -> Vec2 {
        let d = to - from;
        if d.length_squared() < MAX_LENGTH * MAX_LENGTH { to } else { from + d.normalize() * MAX_LENGTH }
    }
}
