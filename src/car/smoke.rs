//! The car's smoke (`Car.MovementControl`, `Car.CreateTrail`,
//! `Car.UpdateCabina`), run on the 60 Hz step as C# did, with its odds
//! per step:
//!
//! - **Wheel streaks** while the wheels slip: a dark streak behind each
//!   rear wheel along the way it slides, one step in four, once moving.
//! - **Exhaust** while the stick is held and the car is slow: a puff shot
//!   backwards from its tail, likelier the slower it goes.
//! - **Idle puffs** now and then while it stands still (cars drawn from
//!   parts only).

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;
use std::f32::consts::PI;

use super::Car;
use crate::particles::styles::{EXHAUST, WHEEL_TRAIL};
use crate::render::particles::Particles;

/// Where the car's smoke comes from, in its own frame (world units, x
/// ahead).
#[derive(Component, Debug, Clone, Copy)]
pub struct SmokePoints {
    /// The left front wheel; the rear ones are its mirror through the
    /// centre (`GetWorldPoint(-_wheelLeftRelativePosition)`).
    pub wheel: Vec2,
    /// The tail (`_backwardPoint`: the radius behind the centre).
    pub tail: Vec2,
    /// Puffs while standing (`LoadWheelBodyColorViews`).
    pub idle: bool,
}

#[derive(Resource)]
pub struct SmokeRng(pub SmallRng);

pub fn car_smoke(
    mut rng: ResMut<SmokeRng>,
    mut particles: ResMut<Particles>,
    cars: Query<(&Car, &SmokePoints, &Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
) {
    let rng = &mut rng.0;
    for (car, points, position, rotation, velocity, spin) in &cars {
        let (pos, angle, v) = (position.0, rotation.as_radians(), velocity.0);
        let world = |local: Vec2| pos + Vec2::from_angle(angle).rotate(local);
        let speed_amount = (v.length_squared() / car.params.max_speed.powi(2)).min(1.0);
        let behind = angle + PI;
        let spread = |rng: &mut SmallRng| (rng.random::<f32>() - 0.5) * 0.2 * PI;
        if car.drive.steering {
            if car.drive.skidding && rng.random_bool(0.25) && speed_amount > 0.1 {
                let rear = [-points.wheel, Vec2::new(-points.wheel.x, points.wheel.y)].map(world);
                // The way the left rear wheel slides.
                let slide = v + spin.0 * (rear[0] - pos).perp();
                let dir = slide.to_angle();
                for at in rear {
                    particles.0.try_add(&WHEEL_TRAIL, at, dir, dir + PI);
                }
            }
            if rng.random::<f32>() < 0.2 - 0.5 * speed_amount {
                let dir = behind + spread(rng);
                particles.0.try_add(&EXHAUST, world(points.tail), dir, rng.random::<f32>() * 2.0 * PI);
            }
        } else if points.idle && v == Vec2::ZERO && rng.random_bool(0.02) {
            let dir = behind + spread(rng);
            particles.0.try_add(&EXHAUST, world(points.tail), dir, rng.random::<f32>() * 2.0 * PI);
        }
    }
}
