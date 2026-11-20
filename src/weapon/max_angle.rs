//! How far a fixed gun may turn: `AdjustWeaponMaxAngle`.
//!
//! C# origin: `ActionRunner.AdjustWeaponMaxAngle` (2013), called when a
//! static simple car is made with a weapon the map names
//! (`EntitiesManager.CreateSimpleCar`) and by the `AdjustWepaonMaxAngle`
//! action. A turret in a nook must not aim into the wall beside it (its aim
//! line would end an arm's length away, and the AI would waste its turning
//! on targets it cannot reach): from the car's heading it casts a ray of
//! `min_distance` every 5° one way and the other, up to the gun's current
//! `MaxAngle`, stops each side at the first ray that meets the walls, and
//! sets `MaxAngle` to the wider of the two free angles.
//!
//! - The rays start at the car's middle, miss every entity (cars, boxes,
//!   doors: only the map's walls) and are [`MapGrid::ray_hits_wall`]'s.
//! - The first ray is 5° off the heading, so a car boxed in straight ahead
//!   gets an angle of 0 and an open one gets 55° of 60° (the last step is
//!   `angle < maxAngle`, never equal; with a float's luck C# may take one
//!   step more, here it never does).
//! - Only a static car with a gun that aims (not the Hammer) is adjusted.
//!   At spawn, only when the map's `Weapon` names one (`Default` leaves the
//!   car's own gun alone, and `None` has no gun then). The action does the
//!   same later, with its `MinDistance` in cells.
//!
//! Not ported: the debug lines drawn at the new limits.

use std::f32::consts::PI;

use avian2d::prelude::*;
use bevy::prelude::*;

use super::Weapon;
use crate::actions::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::AdjustWepaonMaxAngle;
use crate::game::LoadedMap;
use crate::map::grid::MapGrid;

/// `delta`: the step between rays, 5°.
pub const STEP: f32 = PI / 180.0 * 5.0;

/// The new `MaxAngle`: `blocked(offset)` says whether the ray `offset`
/// radians off the heading (either side) meets a wall.
pub fn free_angle(max_angle: f32, blocked: impl Fn(f32) -> bool) -> f32 {
    let side = |sign: f32| {
        let (mut free, mut angle) = (0.0, STEP);
        // `angle < maxAngle`, a millionth spare for the sums.
        while angle < max_angle - 1e-4 {
            if blocked(sign * angle) {
                break;
            }
            free += STEP;
            angle += STEP;
        }
        free
    };
    side(1.0).max(side(-1.0))
}

/// `AdjustWeaponMaxAngle(ent, minDistance)` for a car at `at` (world)
/// heading `heading`: the gun's `MaxAngle` is set. Does nothing for a gun
/// that does not aim.
pub fn adjust(gun: &mut Weapon, grid: &MapGrid, cell: f32, at: Vec2, heading: f32, min_distance: f32) {
    if !gun.kind.is_complex() {
        return;
    }
    let from = [at.x, -at.y];
    let blocked = |offset: f32| {
        let direction = Vec2::from_angle(heading + offset);
        grid.ray_hits_wall(cell, from, [direction.x, -direction.y], min_distance)
    };
    gun.aim.max_angle = free_angle(gun.aim.max_angle, blocked);
}

/// `AdjustWepaonMaxAngle(item, MinDistance)`: for a static car.
fn run_adjust(
    In(run): In<Run<AdjustWepaonMaxAngle>>,
    ids: MapIds,
    map: Res<LoadedMap>,
    mut cars: Query<(&RigidBody, &mut Weapon, &Position, &Rotation)>,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if let Ok((body, mut gun, position, rotation)) = cars.get_mut(e)
        && *body == RigidBody::Static
    {
        // `LevelLengthToWorld`: cells to units.
        let min_distance = run.data.min_distance * map.cell.0;
        adjust(&mut gun, &map.grid, map.cell.0, position.0, rotation.as_radians(), min_distance);
    }
    true
}

pub struct MaxAnglePlugin;

impl Plugin for MaxAnglePlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AdjustWepaonMaxAngle, _>(run_adjust);
    }
}
