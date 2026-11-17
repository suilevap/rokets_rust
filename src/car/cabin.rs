//! The cabin sways on the chassis (`Car.UpdateCabina`, `DrawCabina`).
//!
//! Its front and back hang on two springs (`ElasticVector2`, stiffness 50,
//! damping from the car's `BodyElastic`). Whenever the car speeds up,
//! brakes or turns, the chassis points at the front and the back of the
//! car accelerate and the springs are pushed the other way, so the cabin
//! lags behind, then settles. While the car stands still the springs get
//! tiny random pushes, and the cabin idles as an engine would shake it.
//! The cabin is drawn at the springs' mean, turned by how far the front
//! and back moved apart.
//!
//! The springs move in the world's frame, as in C#; their difference is
//! turned into the car's frame for the cabin's angle (C# added the
//! world-frame difference to the car-frame front point, which only agrees
//! when the car faces right).

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;

use super::smoke::SmokeRng;
use crate::render::layers::Layer;

/// `ElasticVector2`'s pull towards rest, per second².
const STIFFNESS: f32 = 50.0;
/// How strongly the points' acceleration pushes the springs.
const KICK: f32 = 0.005;
/// Random pushes at rest, front and back (`NextVector2Fake() × 0.05/0.1`).
const IDLE_FRONT: f32 = 0.05;
const IDLE_BACK: f32 = 0.1;

/// A damped spring pulling a 2D offset back to zero (`ElasticVector2`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub offset: Vec2,
    pub speed: Vec2,
}

impl Spring {
    /// One step of `dt` seconds; `friction` is the share of speed lost
    /// per second (C#'s `Friction`, applied as `(1 − friction)^dt`).
    pub fn step(&mut self, dt: f32, friction: f32) {
        self.speed -= self.offset * STIFFNESS * dt;
        self.speed *= (1.0 - friction).powf(dt);
        self.offset += self.speed * dt;
    }
}

/// The cabin's two springs, on the car.
#[derive(Component, Debug, Clone)]
pub struct CabinSway {
    pub front: Spring,
    pub back: Spring,
    /// `0.98 + 0.02 × (1 − BodyElastic)`.
    pub friction: f32,
    /// The front and back points, `radius` ahead of and behind the centre.
    pub radius: f32,
    last_front: Vec2,
    last_back: Vec2,
}

impl CabinSway {
    pub fn new(body_elastic: f32, radius: f32) -> CabinSway {
        CabinSway {
            front: Spring::default(),
            back: Spring::default(),
            friction: 0.98 + 0.02 * (1.0 - body_elastic),
            radius,
            last_front: Vec2::ZERO,
            last_back: Vec2::ZERO,
        }
    }

    /// One 60 Hz step for a car at `angle` moving at `velocity`, turning at
    /// `spin`: the springs take the front and back points' acceleration.
    pub fn update(&mut self, dt: f32, angle: f32, velocity: Vec2, spin: f32) {
        // Velocities of the front and back points: v + ω × r.
        let ahead = Vec2::from_angle(angle) * self.radius;
        let front = velocity + spin * ahead.perp();
        let back = velocity - spin * ahead.perp();
        let (front_acc, back_acc) = ((front - self.last_front) / dt, (back - self.last_back) / dt);
        self.last_front = front;
        self.last_back = back;
        // Per step, as C# did once a frame.
        self.front.speed -= front_acc * KICK;
        self.back.speed -= back_acc * KICK;
        self.front.step(dt, self.friction);
        self.back.step(dt, self.friction);
    }

    /// The cabin's offset and turn in the car's frame, for a car at
    /// `angle`.
    pub fn pose(&self, angle: f32) -> (Vec2, f32) {
        let to_car = Vec2::from_angle(-angle);
        let offset = to_car.rotate((self.front.offset + self.back.offset) / 2.0);
        let apart = to_car.rotate(self.front.offset - self.back.offset);
        (offset, (Vec2::new(self.radius, 0.0) + apart).to_angle())
    }
}

/// The cabin sprite of a car.
#[derive(Component, Debug)]
pub struct Cabin;

/// Runs on the 60 Hz step, after the drive.
pub fn sway_cabins(
    time: Res<Time>,
    mut rng: ResMut<SmokeRng>,
    mut cars: Query<(&mut CabinSway, &Rotation, &LinearVelocity, &AngularVelocity, &Children)>,
    mut cabins: Query<&mut Transform, With<Cabin>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let rng = &mut rng.0;
    for (mut sway, rotation, v, spin, children) in &mut cars {
        let angle = rotation.as_radians();
        if v.0 == Vec2::ZERO {
            let mut nudge = |k: f32| Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * k;
            sway.front.speed += nudge(IDLE_FRONT);
            sway.back.speed += nudge(IDLE_BACK);
        }
        sway.update(dt, angle, v.0, spin.0);
        let (offset, turn) = sway.pose(angle);
        for child in children.iter() {
            if let Ok(mut t) = cabins.get_mut(child) {
                t.translation = offset.extend(Layer::CarBody.z());
                t.rotation = Quat::from_rotation_z(turn);
            }
        }
    }
}
