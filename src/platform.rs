//! Moving platforms (`Entities/Components/MovingPlatform.cs`, made by
//! `AddMovingPlatformComponent`, usually at map start).
//!
//! An entity goes back and forth between where it stands and an end: the
//! action's `Position` (map cells), or else its own length ×
//! `OpenDistanceScale` ahead along its facing. Its speed is set every
//! step, so what it meets does not slow it: still for the first half of
//! `PeriodSec`, then out in half a period, back in the next, and so on.
//! It does not bounce. A static body does not move, as in C#.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::actions::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::AddMovingPlatformComponent;
use crate::game::{GameState, LoadedMap};

#[derive(Component, Debug)]
pub struct MovingPlatform {
    /// Out from the start, units a second.
    speed: Vec2,
    /// 0 at first, then 1 out and -1 back.
    direction: f32,
    half_period: f32,
    until_turn: f32,
}

impl MovingPlatform {
    /// One step of `dt` seconds: the speed to have (`MovingPlatform.Update`).
    pub fn step(&mut self, dt: f32) -> Vec2 {
        self.until_turn -= dt;
        if self.until_turn <= 0.0 {
            self.until_turn += self.half_period;
            self.direction = if self.direction == 0.0 { 1.0 } else { -self.direction };
        }
        self.speed * self.direction
    }

    pub fn new(start: Vec2, end: Vec2, period: f32) -> Self {
        let half_period = (period / 2.0).max(f32::EPSILON);
        MovingPlatform {
            speed: (end - start) / half_period,
            direction: 0.0,
            half_period,
            until_turn: half_period,
        }
    }
}

pub struct PlatformPlugin;

impl Plugin for PlatformPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AddMovingPlatformComponent, _>(add_platform)
            .add_systems(FixedUpdate, move_platforms.run_if(in_state(GameState::Playing)));
    }
}

fn add_platform(
    In(run): In<Run<AddMovingPlatformComponent>>,
    ids: MapIds,
    map: Res<LoadedMap>,
    bodies: Query<(&Position, &Rotation, &Collider)>,
    mut commands: Commands,
) -> bool {
    let d = &run.data;
    let Some(e) = ids.entity(run.target(d.item_id)) else {
        return false;
    };
    let Ok((pos, rot, collider)) = bodies.get(e) else {
        return false;
    };
    let end = if d.position.x == 0.0 && d.position.y == 0.0 {
        let length = collider.shape_scaled().as_cuboid().map_or(1.0, |c| c.half_extents.x * 2.0);
        pos.0 + Vec2::from_angle(rot.as_radians()) * length * d.open_distance_scale
    } else {
        map.world(d.position.x, d.position.y)
    };
    commands.entity(e).insert((MovingPlatform::new(pos.0, end, d.period_sec), Restitution::ZERO));
    true
}

fn move_platforms(time: Res<Time>, mut platforms: Query<(&mut MovingPlatform, &mut LinearVelocity)>) {
    for (mut p, mut v) in &mut platforms {
        v.0 = p.step(time.delta_secs());
    }
}
