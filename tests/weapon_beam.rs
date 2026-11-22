//! A beam's path (`weapon::beam::trace`) in a small physics world: the
//! Railgun's bullet, the aim line and the Laser's beam share it.

use std::f32::consts::FRAC_PI_4;

use avian2d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use tilerider::weapon::beam::{BeamRules, Leg, Reach, trace};

/// Marks what can be hurt in these tests.
#[derive(Component)]
struct Hurtable;

fn world_with(walls: &[(Vec2, Vec2, bool)]) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()));
    for &(at, size, hurt) in walls {
        let mut e = app.world_mut().spawn((RigidBody::Static, Collider::rectangle(size.x, size.y), Transform::from_translation(at.extend(0.0))));
        if hurt {
            e.insert(Hurtable);
        }
    }
    // Let the physics set up its colliders and the spatial index.
    for _ in 0..3 {
        app.update();
    }
    app
}

fn legs(app: &mut App, direction: f32, rules: BeamRules) -> Vec<Leg> {
    let shooter = app.world_mut().spawn_empty().id();
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery, hurt: Query<(), With<Hurtable>>| {
            let passes = |_| false;
            let can_hurt = |e| hurt.contains(e);
            trace(&spatial, shooter, &passes, &can_hurt, Vec2::ZERO, Vec2::ZERO, direction, rules)
        })
        .unwrap()
}

const BULLET: BeamRules = BeamRules { reach: Reach::EachLeg(32.0), bounces: 1, stop_at_hurt: false };

#[test]
fn a_slanted_hit_bounces_off_the_surface() {
    // A wall along x = 10, fired at 45° up and to the right.
    let mut app = world_with(&[(Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0), false)]);
    let legs = legs(&mut app, FRAC_PI_4, BULLET);
    assert_eq!(legs.len(), 2, "{legs:?}");
    assert!((legs[0].end.x - 10.0).abs() < 1e-3, "{:?}", legs[0].end);
    // Mirrored: now up-left, from 0.1 off the wall, its full reach.
    let back = Vec2::from_angle(legs[1].direction);
    assert!((back - Vec2::new(-1.0, 1.0).normalize()).length() < 1e-3, "{back}");
    assert!((legs[1].cast_from.x - 9.9).abs() < 1e-3);
    assert!((legs[1].end.distance(legs[1].cast_from) - 32.0).abs() < 1e-3);
}

#[test]
fn a_head_on_hit_stops_the_beam() {
    let mut app = world_with(&[(Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0), false)]);
    // Within 15° of the normal: no bounce.
    assert_eq!(legs(&mut app, 0.2, BULLET).len(), 1);
}

#[test]
fn the_aim_line_stops_at_what_can_be_hurt_and_shares_its_range() {
    let wall = (Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0), true);
    let line = BeamRules { reach: Reach::Total(32.0), bounces: 1, stop_at_hurt: true };
    let mut app = world_with(&[wall]);
    assert_eq!(legs(&mut app, FRAC_PI_4, line).len(), 1);
    // The bullet bounces off it anyway.
    assert_eq!(legs(&mut app, FRAC_PI_4, BULLET).len(), 2);
    // Off a plain wall, the bounce gets what is left of the 32.
    let mut app = world_with(&[(Vec2::new(11.0, 0.0), Vec2::new(2.0, 100.0), false)]);
    let l = legs(&mut app, FRAC_PI_4, line);
    let first = l[0].start.distance(l[0].end);
    assert!((l[1].start.distance(l[1].end) - (32.0 - first)).abs() < 0.2, "{l:?}");
}
