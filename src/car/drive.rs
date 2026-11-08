//! The car's driving model, ported from `Car.MovementControl`,
//! `Car.Brake`, `Car.AdditionalFriction` and `ControlableEntity`.
//!
//! The player does not steer and throttle: they point where they want to
//! go (`move_dir`, length 0..1) and the car turns towards it and pushes.
//! - Pointing roughly ahead or behind (within 45°): push along the stick
//!   and along the car's nose, at a point in front of the centre, so the
//!   car also swings round — until the speed for this stick length is
//!   reached.
//! - Pointing sideways: build up a turning speed that decays (×0.8 per
//!   step), with a small push along the nose.
//! - No input: brake (speed loses a fixed amount per axis per step).
//! - Always: sideways slip loses speed (`AdditionalFriction`).
//!
//! The C# game ran this once per 60 Hz frame and its numbers are per
//! frame, so this runs in `FixedUpdate` at 60 Hz too. It is a pure function
//! of the body's state so it can be tested without the physics engine.

use bevy::math::Vec2;
use std::f32::consts::PI;

use crate::content::cars::CarData;
use crate::units::px;

/// The C# frame time (`Settings.UpdateTimeSec`).
pub const STEP_SECONDS: f32 = 1.0 / 60.0;

/// `Car.InactiveControlRate`: shorter sticks are ignored.
const INACTIVE_CONTROL_RATE: f32 = 0.25;
/// `ControlableEntity.AdditionalDragCoefficientDefault`.
const ADDITIONAL_DRAG: f32 = 0.025;
/// `Car._rotationAttenuation`.
const ROTATION_ATTENUATION: f32 = 0.8;
/// `Entity.MaxSpeedDefault`: 1500 px/s.
const MAX_SPEED_DEFAULT_PX: f32 = 1500.0;
/// `ControlableEntity._accelerationForceDefault`: 2048 px.
const ACCELERATION_FORCE_DEFAULT_PX: f32 = 2048.0;

/// The car's tuning, from `Cars.xml` and the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriveParams {
    pub acceleration_force: f32,
    pub max_speed: f32,
    pub rotation_acceleration: f32,
    pub mass: f32,
    /// Distance from the centre to the push point (half the sprite's longer side).
    pub radius: f32,
    /// The map's `Friction`.
    pub map_friction: f32,
}

impl DriveParams {
    /// `radius` is in world units, from the collider.
    pub fn new(car: &CarData, radius: f32, map_friction: f32) -> Self {
        DriveParams {
            acceleration_force: px(ACCELERATION_FORCE_DEFAULT_PX) * car.acceleration,
            max_speed: px(MAX_SPEED_DEFAULT_PX) * 0.5 * car.speed,
            rotation_acceleration: 0.02 * car.controllability,
            mass: car.mass,
            radius,
            map_friction,
        }
    }
}

/// What persists between steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DriveState {
    rotation_speed: f32,
    /// Angle between the nose and the stick, for drawing the front wheels.
    pub wheel_angle: f32,
}

/// The body as the drive sees it, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub angle: f32,
    pub velocity: Vec2,
    pub angular_velocity: f32,
}

/// A force at a point in the body's local frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalForce {
    pub force: Vec2,
    pub local_point: Vec2,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DriveOutput {
    pub forces: Vec<LocalForce>,
    /// The velocity after braking and slip friction.
    pub velocity: Vec2,
    pub angular_velocity: f32,
    /// True when the wheels slip (for tyre marks and sound later).
    pub skidding: bool,
}

/// `MathHelper.DirectionDiff`: the signed smallest angle from `b` to `a`.
pub fn direction_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % (2.0 * PI);
    if d > PI {
        d -= 2.0 * PI;
    } else if d < -PI {
        d += 2.0 * PI;
    }
    d
}

fn angle_of(v: Vec2) -> f32 {
    v.y.atan2(v.x)
}

/// One 60 Hz step of the drive. `move_dir` is the stick in world space.
pub fn drive_step(p: &DriveParams, state: &mut DriveState, body: Body, move_dir: Vec2) -> DriveOutput {
    let mut out = DriveOutput {
        velocity: body.velocity,
        angular_velocity: body.angular_velocity,
        ..Default::default()
    };
    let forward = Vec2::from_angle(body.angle);
    let front_point = Vec2::new(p.radius, 0.0);
    let stick = move_dir.length();

    // Car.ControlHandle: no input brakes.
    if move_dir == Vec2::ZERO {
        let friction = 8.0 * STEP_SECONDS * p.map_friction;
        let v = out.velocity;
        out.velocity = if v.x.abs() > friction || v.y.abs() > friction {
            Vec2::new(v.x - v.x.signum() * friction, v.y - v.y.signum() * friction)
        } else {
            Vec2::ZERO
        };
    }

    // Car.MovementControl.
    state.wheel_angle = 0.0;
    if stick > INACTIVE_CONTROL_RATE {
        let speed_rate = (out.velocity.length_squared() / (p.max_speed * p.max_speed * stick * stick)).min(1.0);
        let max_speed_reached = speed_rate >= 1.0;
        let control_angle = angle_of(move_dir);
        let diff = direction_diff(control_angle, body.angle);
        state.wheel_angle = diff;
        let diff_abs = diff.abs();
        if !(0.25 * PI..=0.75 * PI).contains(&diff_abs) {
            if !max_speed_reached {
                let force = move_dir * p.acceleration_force * 0.5;
                out.forces.push(LocalForce {
                    force,
                    local_point: front_point,
                });
                if out.velocity != Vec2::ZERO {
                    let sign = forward.dot(move_dir).signum();
                    let force = forward * p.acceleration_force * 0.5 * sign * stick;
                    out.forces.push(LocalForce {
                        force,
                        local_point: front_point,
                    });
                    let next = force / p.mass * 0.5 + out.velocity;
                    let useful = direction_diff(angle_of(next), control_angle).cos();
                    out.skidding = useful < 0.5;
                } else {
                    out.skidding = true;
                }
            }
        } else {
            let rotation_accel = p.rotation_acceleration
                * (0.01 + stick * 0.5 + out.velocity.length() / p.max_speed * 0.49);
            state.rotation_speed += diff.signum() * rotation_accel * STEP_SECONDS * 1000.0;
            out.skidding = true;
        }
    }
    if state.rotation_speed.abs() > 0.001 {
        let force = forward * p.acceleration_force / 4.0;
        let sign = force.dot(move_dir).signum();
        out.forces.push(LocalForce {
            force: force * sign,
            local_point: front_point,
        });
    }
    out.angular_velocity += state.rotation_speed;
    state.rotation_speed *= ROTATION_ATTENUATION;

    // Car.AdditionalFriction: slipping sideways costs speed.
    if out.velocity != Vec2::ZERO {
        let slip = direction_diff(body.angle, angle_of(out.velocity)).sin().abs();
        out.velocity *= 1.0 - slip * ADDITIONAL_DRAG * p.map_friction;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> DriveParams {
        DriveParams {
            acceleration_force: px(2048.0),
            max_speed: px(1500.0) * 0.5 * 0.6,
            rotation_acceleration: 0.02,
            mass: 2.97,
            radius: 1.0,
            map_friction: 1.0,
        }
    }

    #[test]
    fn pushes_forward_when_pointing_ahead() {
        let mut s = DriveState::default();
        let body = Body {
            angle: 0.0,
            velocity: Vec2::ZERO,
            angular_velocity: 0.0,
        };
        let out = drive_step(&params(), &mut s, body, Vec2::X);
        assert_eq!(out.forces.len(), 1);
        assert!(out.forces[0].force.x > 0.0);
    }

    #[test]
    fn turns_when_pointing_sideways() {
        let mut s = DriveState::default();
        let body = Body {
            angle: 0.0,
            velocity: Vec2::ZERO,
            angular_velocity: 0.0,
        };
        // Stick up (+y, counter-clockwise) turns the car counter-clockwise.
        let out = drive_step(&params(), &mut s, body, Vec2::Y);
        assert!(out.angular_velocity > 0.0);
    }

    #[test]
    fn brakes_without_input() {
        let mut s = DriveState::default();
        let body = Body {
            angle: 0.0,
            velocity: Vec2::new(1.0, 0.0),
            angular_velocity: 0.0,
        };
        let out = drive_step(&params(), &mut s, body, Vec2::ZERO);
        assert!(out.velocity.x < 1.0 && out.velocity.x > 0.0);
        let stopped = drive_step(
            &params(),
            &mut s,
            Body {
                velocity: Vec2::new(0.05, 0.0),
                ..body
            },
            Vec2::ZERO,
        );
        assert_eq!(stopped.velocity, Vec2::ZERO);
    }

    #[test]
    fn stops_pushing_at_top_speed() {
        let p = params();
        let mut s = DriveState::default();
        let body = Body {
            angle: 0.0,
            velocity: Vec2::new(p.max_speed, 0.0),
            angular_velocity: 0.0,
        };
        let out = drive_step(&p, &mut s, body, Vec2::X);
        assert!(out.forces.is_empty());
    }
}
