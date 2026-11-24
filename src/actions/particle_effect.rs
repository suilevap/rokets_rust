//! `CreateParticleEffect`: a one-off particle effect on an entity.
//!
//! C# origin: `ActionRunner.CreateParticleEffectRun`, with
//! `ParticleSystemManager.CreateNewZombieFriend` (March 2014, "Add action to
//! create particle effect"; Swarm's `CreateNewFriendEffect`, which its
//! `TryToChangeToLocalControl` sequence runs for the car that has just
//! joined the player's horde).
//!
//! The only `Effect` there is, `NewZombieFriend`, puts the zombie border
//! sprite ([`NEW_ZOMBIE_FRIEND`]) at the target entity, turned the way it
//! faces, where it swells from 0.65 to 12 times its size and fades in
//! 0.3 s, in the additive pool, which is not replaced when it is full
//! (`AddParticle`, not `TryAddParticle`). `ItemId` is the target, or the
//! entity that set the action off when it is 0 (`GetEntity`). The action
//! always succeeds, whether or not it found anything to draw on (C#
//! `Register`, not `RegisterWithResult`), so a chained `NextAction` runs.
//!
//! The effect is drawn only when the camera sees the entity, within 4 units
//! of the picture's edge (`Camera.IsInView(pos, 4)`), as C# skipped it
//! off screen. It is a view effect with no part in the game's rules, so
//! it is made from the action's system (`FixedUpdate`) like the other
//! effects (`Particles`), with nothing random in it.
//!
//! Not ported: `ent.Removed` (an entity that is gone is not in the world
//! here, so it is the same as no entity); an `Effect` other than
//! `NewZombieFriend` (C#'s `switch` had no other case, and there is no
//! other value).

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{CreateParticleEffect, EffectType};
use crate::particles::styles::NEW_ZOMBIE_FRIEND;
use crate::render::camera::MainCamera;
use crate::render::particles::Particles;
use crate::weapon::aim_help::in_view;

/// `Camera.IsInView(pos, 4)`: how far outside the picture the effect is
/// still made, in world units.
pub const VIEW_MARGIN: f32 = 4.0;

/// What the effect draws, to load with a map that has the action.
static SPRITES: [&str; 1] = [NEW_ZOMBIE_FRIEND.texture];

pub struct ParticleEffectActionsPlugin;

impl Plugin for ParticleEffectActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<CreateParticleEffect, _>(create_particle_effect)
            .add_action_sprites::<CreateParticleEffect>(&SPRITES);
    }
}

fn create_particle_effect(
    In(run): In<Run<CreateParticleEffect>>,
    ids: MapIds,
    things: Query<&GlobalTransform>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    particles: Option<ResMut<Particles>>,
) -> bool {
    let Some(thing) = ids.entity(run.target(run.data.item_id)).and_then(|e| things.get(e).ok()) else {
        return true;
    };
    let Some(mut particles) = particles else {
        return true;
    };
    let at = thing.translation().truncate();
    // The entity's `Direction`: the way it faces.
    let facing = (thing.rotation() * Vec3::X).truncate().to_angle();
    match run.data.effect {
        EffectType::NewZombieFriend => {
            if cameras.single().is_ok_and(|(c, t)| !in_view(c, t, at, VIEW_MARGIN)) {
                return true;
            }
            particles.add(&NEW_ZOMBIE_FRIEND, at, 0.0, facing);
        }
    }
    true
}
