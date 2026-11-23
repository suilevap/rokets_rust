//! `MoveEntity`: put an entity somewhere else, standing still.
//!
//! C# origin: `ActionRunner.MoveEntityRun` (added in 2012 with
//! `ResetEntityPosition`, "to avoid issues with respawned bots": it is how
//! a map brings a clone back where it belongs). Garage's turrets use it:
//! each runs it as its `ActionOnCreate`, so a clone made at the area's
//! spot (`CloneTur3007`) is put at the turret's post, turned to face the
//! way it fires, and its `NextAction` then locks its body and pushes it.
//!
//! The entity (`ItemId`, or the actor for 0 or less) is teleported, not
//! moved over time and not relative: its body goes to `Position` (map
//! cells, as every map point: `LevelPositionToWorld`) turned to
//! `DirectionDeg`, with no linear or angular speed left, so whatever it
//! was doing it starts from rest. A car with a behaviour-tree control also
//! forgets its target, path, leader and what it saw
//! ([`AiBlackboard::reset`]), as a bot put back at its post should.
//!
//! Like every `void` handler of C# it succeeds when there is no such
//! entity, so a `NextAction` still follows.
//!
//! Not ported: nothing of `MoveEntityRun` is left out. Differences: the
//! entity's `Transform` is set with its `Position` and `Rotation` (Avian
//! takes a body's pose from the `Transform` on its first physics step, which
//! would undo a move made at map start), and `TransformInterpolation` eases
//! the entity across over the one fixed step the move takes, as it does a
//! teleport's arrival. The camera is not moved (C# did not either).

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::ai::AiBlackboard;
use crate::content::actions::defs::MoveEntity;
use crate::game::LoadedMap;
use crate::units::map_degrees_to_world;

pub struct MoveEntityPlugin;

impl Plugin for MoveEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<MoveEntity, _>(move_entity);
    }
}

/// What can stand somewhere: its body, its speeds if it has them, and the
/// brain of a behaviour-tree control.
type Body<'a> = (
    &'a mut Position,
    &'a mut Rotation,
    Option<&'a mut LinearVelocity>,
    Option<&'a mut AngularVelocity>,
    Option<&'a mut AiBlackboard>,
    Option<&'a mut Transform>,
);

fn move_entity(
    In(run): In<Run<MoveEntity>>,
    ids: MapIds,
    map: Res<LoadedMap>,
    mut bodies: Query<Body>,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return true;
    };
    let Ok((mut position, mut rotation, velocity, spin, brain, transform)) = bodies.get_mut(e) else {
        return true;
    };
    let at = run.data.position;
    let place = map.world(at.x, at.y);
    // C# set `-DirectionDeg` in its y-down space: `+DirectionDeg` here.
    let angle = map_degrees_to_world(run.data.direction_deg);
    position.0 = place;
    *rotation = Rotation::radians(angle);
    // Avian takes a body's first pose from its `Transform`, ignoring a
    // `Position` set before the first physics step (a turret moved by its
    // `ActionOnCreate` at map start); keep the two the same.
    if let Some(mut t) = transform {
        t.translation = place.extend(t.translation.z);
        t.rotation = Quat::from_rotation_z(angle);
    }
    if let Some(mut v) = velocity {
        v.0 = Vec2::ZERO;
    }
    if let Some(mut w) = spin {
        w.0 = 0.0;
    }
    if let Some(mut brain) = brain {
        brain.reset();
    }
    true
}
