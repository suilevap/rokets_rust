//! A path to follow: points, and a carrot that runs ahead of the car along
//! them.
//!
//! C# origin: `AI/Navigation/NavPath.cs` (2013). A path is a list of points
//! and a *current position* on it, which [`NavPath::update`] moves along the
//! path at the car's speed, keeping it between a minimum and a maximum
//! distance ahead of the car (`MoveBase.Tick`); the car steers at it. A
//! closed path wraps round; an open one ends (`EndPath`), and the car has
//! arrived once it is near that end.
//!
//! Positions are world units (y up, like the graph's).
//!
//! Differences from C#:
//! - Stepping back from the second segment into the first puts the carrot
//!   where the distance says, on the first segment. C# put it on the second
//!   point and reset the distance to 0 (its backward loop's `else` took the
//!   point it was leaving), so the carrot jumped forward and the distance
//!   no longer matched it.
//! - [`NavPath::update`] measures the car's distance to the carrot again
//!   after the carrot has moved on, before it decides whether the car has
//!   fallen too far behind. C# used the distance it had measured before the
//!   move, so a step forward was never pulled back.
//!
//! Not ported: `Draw` (debug drawing: see `ai::navigation`).

use bevy::math::Vec2;

use super::graph::Blocker;

/// `NavPath`.
#[derive(Clone, Debug, PartialEq)]
pub struct NavPath {
    current_index: i32,
    path: Vec<Vec2>,
    partial_distance: f32,
    full_length: f32,
    distance: f32,
    end_path: bool,
    current_position: Vec2,
    closed: bool,
    blockers: Vec<Option<Blocker>>,
}

impl NavPath {
    /// `NavPath(path, closed)`.
    pub fn new(path: Vec<Vec2>, closed: bool) -> NavPath {
        NavPath::with_blockers(path, Vec::new(), closed)
    }

    /// `NavPath(path, blockingComponents, closed)`: `blockers` are what the
    /// links the path takes are closed by (`GetPath` found them blocked).
    pub fn with_blockers(path: Vec<Vec2>, blockers: Vec<Option<Blocker>>, closed: bool) -> NavPath {
        let full_length: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
        NavPath {
            current_index: 0,
            current_position: path.first().copied().unwrap_or(Vec2::ZERO),
            end_path: full_length == 0.0,
            partial_distance: 0.0,
            full_length,
            distance: 0.0,
            path,
            closed,
            blockers,
        }
    }

    /// `FullLength`.
    pub fn full_length(&self) -> f32 {
        self.full_length
    }

    /// `Length`: what is left to go.
    pub fn length(&self) -> f32 {
        self.full_length - self.distance
    }

    /// `Points`.
    pub fn points(&self) -> &[Vec2] {
        &self.path
    }

    /// `Distance`: how far the carrot has gone.
    pub fn distance(&self) -> f32 {
        self.distance
    }

    /// `StartPath`: the carrot is at the start.
    pub fn is_start(&self) -> bool {
        self.distance == 0.0
    }

    /// `EndPath`: the carrot has reached the end of an open path.
    pub fn is_end(&self) -> bool {
        self.end_path
    }

    /// `CurrentPosition`: the carrot.
    pub fn current_position(&self) -> Vec2 {
        self.current_position
    }

    /// `StartPosition` (the origin for a path with no points, where C#
    /// threw).
    pub fn start_position(&self) -> Vec2 {
        self.path.first().copied().unwrap_or(Vec2::ZERO)
    }

    /// `LastPosition` (the origin for a path with no points).
    pub fn last_position(&self) -> Vec2 {
        self.path.last().copied().unwrap_or(Vec2::ZERO)
    }

    /// `Closed`.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// `Blockers`: what closes the links the path takes, in order; empty for
    /// a free way.
    pub fn blockers(&self) -> &[Option<Blocker>] {
        &self.blockers
    }

    /// `Smooth`: each inner point moved to the middle of a Hermite curve
    /// through its neighbours (unused in C#, where the call is commented
    /// out).
    pub fn smooth(data: &[Vec2]) -> Vec<Vec2> {
        if data.len() < 3 {
            return data.to_vec();
        }
        let mut result = vec![data[0]];
        for i in 1..data.len() - 1 {
            // `Vector2.Hermite(p[i-1], p[i]-p[i-1], p[i], p[i+1]-p[i], 0.5)`.
            let (v1, t1, v2, t2) = (data[i - 1], data[i] - data[i - 1], data[i], data[i + 1] - data[i]);
            result.push(v1 * 0.5 + v2 * 0.5 + t1 * 0.125 - t2 * 0.125);
        }
        result.push(data[data.len() - 1]);
        result
    }

    /// `AddPoint`: the path goes on to `position`, unless that is no more
    /// than a unit from where it ends.
    pub fn add_point(&mut self, position: Vec2) {
        let d = self.path.last().map_or(0.0, |last| last.distance(position));
        if d > 1.0 || self.path.is_empty() {
            self.full_length += d;
            self.path.push(position);
            self.end_path = false;
            if self.path.len() == 1 {
                self.current_position = self.path[0];
            }
        }
    }

    /// `Update(position, dist, minDistance, maxDistance)`: the carrot first
    /// catches up if the car is within `min_distance` of it (a quarter of
    /// `min_distance` at a time, up to 8 times), moves `dist` on if the car
    /// is under `max_distance` from it, and is pulled back if the car has
    /// fallen further behind than that. Returns whether the path goes on.
    pub fn update(&mut self, position: Vec2, dist: f32, min_distance: f32, max_distance: f32) -> bool {
        let mut d = self.current_position.distance(position);
        let mut i = 8;
        while d < min_distance && !self.end_path && i > 0 {
            self.update_position(min_distance / 4.0);
            d = self.current_position.distance(position);
            i -= 1;
        }
        if d < max_distance && !self.end_path {
            self.update_position(dist);
            d = self.current_position.distance(position);
        }
        // Too far ahead of the car: back, a step at a time while that brings
        // it nearer (a step that takes it further off is undone).
        while d > max_distance && !self.is_start() {
            self.update_position(-min_distance / 4.0);
            let nearer = self.current_position.distance(position);
            if nearer >= d {
                if nearer > d {
                    self.update_position(min_distance / 4.0);
                }
                break;
            }
            d = nearer;
        }
        !self.end_path
    }

    /// `UpdatePosition(deltaDistance)`: moves the carrot along the path by
    /// `delta` (back when negative). Returns whether it has not reached the
    /// end.
    pub fn update_position(&mut self, delta: f32) -> bool {
        let count = self.path.len() as i32;
        if self.end_path && delta > 0.0 {
            return false;
        }
        if self.end_path && delta < 0.0 {
            self.end_path = false;
        }
        if count < 2 {
            self.end_path = true;
            return false;
        }
        self.distance += delta;
        self.partial_distance += delta;
        let at = |i: i32| self.path[i.rem_euclid(count) as usize];
        // Back over the segments the carrot has left, to the start at most.
        while self.partial_distance < 0.0 {
            if self.current_index == 0 {
                self.partial_distance = 0.0;
                self.distance = 0.0;
                break;
            }
            self.current_index -= 1;
            self.partial_distance += at(self.current_index).distance(at(self.current_index + 1));
        }
        let mut curr = at(self.current_index);
        let mut next = at(self.current_index + 1);
        let mut d = curr.distance(next);
        while self.partial_distance >= d {
            self.partial_distance -= d;
            self.current_index = (self.current_index + 1) % count;
            if self.current_index < count - 1 || self.closed {
                curr = next;
                next = self.path[((self.current_index + 1) % count) as usize];
                d = curr.distance(next);
            } else {
                self.partial_distance = d;
                self.current_position = next;
                self.current_index = count - 2;
                self.end_path = true;
                break;
            }
        }
        if !self.end_path {
            self.current_position = curr.lerp(next, self.partial_distance / d);
        }
        !self.end_path
    }

    /// `Reset`: back to the start.
    pub fn reset(&mut self) {
        self.partial_distance = 0.0;
        self.current_position = self.start_position();
        self.current_index = 0;
        self.end_path = false;
    }
}
