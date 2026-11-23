//! A static car the magnet tows (`ControlableEntity.OnLinkTo`): a turret
//! (`CanMove` false, `CanLink` true) is a static body until a magnet takes
//! it, dynamic while it is on the rope, and static again, where it ended
//! up, once it is let go. `ApplyGadgetToPosition` is how Derby's truck
//! takes its turret and GarbageCollector's takes the Tesla one.
//!
//! Not ported: C# switched back to static one update after the rope was
//! gone ("to avoid collision static obj with walls"); here it is the
//! same fixed step.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::Magnet;

/// A body that is static while no magnet has it (`IsStatic` of a car that
/// cannot move but can be linked).
#[derive(Component, Debug, Default)]
pub struct StaticWhenFree;

/// Static or dynamic by whether a magnet has it on its rope; one that is
/// let go stands still where it is.
pub fn wake_towed(
    mut commands: Commands,
    magnets: Query<&Magnet>,
    mut bodies: Query<(Entity, &RigidBody, &mut LinearVelocity, &mut AngularVelocity), With<StaticWhenFree>>,
) {
    let towed: Vec<Entity> = magnets.iter().filter_map(Magnet::towing).collect();
    for (e, body, mut v, mut w) in &mut bodies {
        if towed.contains(&e) {
            if *body != RigidBody::Dynamic {
                commands.entity(e).insert(RigidBody::Dynamic);
            }
        } else if *body != RigidBody::Static {
            commands.entity(e).insert(RigidBody::Static);
            v.0 = Vec2::ZERO;
            w.0 = 0.0;
        }
    }
}
