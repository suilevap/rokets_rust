//! How a simple car drives (`ControlableEntity.MovementControl`): the
//! model of the map's `SimpleCars`, which is not the player's
//! (`Car.MovementControl`, [`super::drive`]).
//!
//! The stick (a world vector, 0..1) pushes the car along it with one force
//! at a point half a radius ahead of its middle, `acceleration` × 64 × ½ at
//! the most, for as long as the car is slower than its top speed in that
//! direction (`SpeedControlAmountNormalized`). With a `Controllability`
//! the sideways part of its speed is pushed against, so the car slides
//! less. It does not brake, steer or skid. A car that cannot move
//! (`CanMove` false: turrets, static targets) has no [`SimpleDrive`] at all.
//!
//! The stick is written by the car's control ([`crate::ai`]: a turret that
//! can move chases its target); a car with none coasts to a stop (damping)
//! and is shoved around.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::Car;
use super::control::Team;
use crate::controls::PlayerIntent;

/// `ConvertUnits.ToSimUnits(1024 * 2)`: `_accelerationForceDefault` before
/// the car's `Acceleration`.
const ACCELERATION_FORCE: f32 = 1024.0 * 2.0 / 32.0;
/// `Entity.MaxSpeedDefault`, `ToSimUnits(1500)`, before `0.5 × Speed`.
const MAX_SPEED: f32 = 1500.0 / 32.0;

/// A simple car's driving numbers.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SimpleDrive {
    /// `MaxSpeed`: `1500/32 × 0.5 × Speed`.
    pub max_speed: f32,
    /// `AccelerationForce`: `64 × Acceleration`.
    pub force: f32,
    /// `Controllability`: how hard the sideways speed is pushed against.
    pub controllability: f32,
    /// Where the force acts, ahead of the middle (`_dragPoint`: half the
    /// radius).
    pub drag_point: f32,
}

impl SimpleDrive {
    /// From a car's `Speed`, `Acceleration` and `Controllability`, and its
    /// radius (half the longer side of its sprite).
    pub fn new(speed: f32, acceleration: f32, controllability: f32, radius: f32) -> SimpleDrive {
        SimpleDrive {
            max_speed: MAX_SPEED * 0.5 * speed,
            force: ACCELERATION_FORCE * acceleration,
            controllability,
            drag_point: radius / 2.0,
        }
    }

    /// `SpeedControlAmountNormalized`: how much of its top speed (along
    /// `control`) the car has, at most 1.
    fn speed_share(&self, velocity: Vec2, control: Vec2) -> f32 {
        let share = velocity.length_squared() / (self.max_speed * self.max_speed * control.length_squared());
        share.min(1.0)
    }

    /// The force to push the car with now (world), or none: no stick, or
    /// already at its top speed. `MovementControl`.
    pub fn force(&self, velocity: Vec2, control: Vec2) -> Option<Vec2> {
        if control == Vec2::ZERO || self.speed_share(velocity, control) >= 1.0 {
            return None;
        }
        let max_force = self.force * 2.0 / 4.0;
        if velocity != Vec2::ZERO && self.controllability != 0.0 {
            // C# turned the stick a quarter by (−y, x) in its y-down
            // space: ours is (y, −x).
            let across = Vec2::new(control.y, -control.x);
            let drift = velocity.normalize().dot(across) * self.controllability;
            Some((control * (1.0 - drift) - across * drift) * max_force)
        } else {
            Some(control * max_force)
        }
    }
}

/// `Entity.MaxAngularVelocityDefault`: no entity spins faster (the
/// player's car is limited in [`super::drive_cars`]).
const MAX_ANGULAR_VELOCITY: f32 = 50.0;

/// `Entity.Update`: a simple car's spin is kept to 50 rad/s.
pub fn limit_spin(mut cars: Query<&mut AngularVelocity, (With<Team>, Without<Car>)>) {
    for mut spin in &mut cars {
        spin.0 = spin.0.clamp(-MAX_ANGULAR_VELOCITY, MAX_ANGULAR_VELOCITY);
    }
}

/// `ControlHandle` for the cars that can move: push them as their stick
/// says.
pub fn drive_simple_cars(mut cars: Query<(Forces, &SimpleDrive, &PlayerIntent, &RigidBody)>) {
    for (mut forces, drive, intent, body) in &mut cars {
        if !body.is_dynamic() {
            continue;
        }
        let Some(force) = drive.force(forces.linear_velocity(), intent.move_dir) else {
            continue;
        };
        let angle = forces.rotation().as_radians();
        let point = forces.position() + Vec2::from_angle(angle) * drive.drag_point;
        forces.apply_force_at_point(force, point);
    }
}
