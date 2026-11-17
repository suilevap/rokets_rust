//! Private details of the drive step.

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

#[test]
fn wheels_slip_when_turning_or_pushing_against_the_motion() {
    let mut s = DriveState::default();
    let moving = Body {
        angle: 0.0,
        velocity: Vec2::new(5.0, 0.0),
        angular_velocity: 0.0,
    };
    drive_step(&params(), &mut s, moving, Vec2::X);
    assert!(s.steering && !s.skidding, "pushing along the motion grips");
    drive_step(&params(), &mut s, moving, Vec2::Y);
    assert!(s.steering && s.skidding, "turning hard slips");
    drive_step(&params(), &mut s, moving, Vec2::ZERO);
    assert!(!s.steering && !s.skidding, "no stick, no slip");
}
