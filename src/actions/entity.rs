//! Actions on one entity (`ActionRunner`): activate, destroy, push, hurt.

use super::{AddMapAction, MapIds, OnDestroy, Run, RunAction, destroy_action};
use crate::controls::LocalPlayer;
use crate::game::MapId;
use crate::content::actions::defs::{ActivateEntity, ApplyImpulse, DamageEntity, DestroyEntity};
use crate::explosion::{Destroyed, Explosion, Explosive, Health, Hit};
use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

/// Out of the game for now (`Entity.Deactivate`): hidden, out of the physics.
#[derive(Component, Debug)]
pub struct Inactive;

pub struct EntityActionsPlugin;

impl Plugin for EntityActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<ActivateEntity, _>(set_active)
            .add_map_action::<DestroyEntity, _>(destroy)
            .add_map_action::<ApplyImpulse, _>(impulse)
            .add_map_action::<DamageEntity, _>(damage);
    }
}

/// `ActivateEntityActionData`, `ActivateEntity(id)`, `DeactivateEntity(id)`.
fn set_active(In(run): In<Run<ActivateEntity>>, ids: MapIds, mut commands: Commands) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if run.data.deactivate {
        commands.entity(e).insert((
            Inactive,
            RigidBodyDisabled,
            ColliderDisabled,
            Visibility::Hidden,
        ));
    } else {
        commands
            .entity(e)
            .remove::<(Inactive, RigidBodyDisabled, ColliderDisabled)>()
            .insert(Visibility::Inherited);
    }
    true
}

/// `DestroyEntityActionData`, `DestroyEntity(id)`: `Entity.Destroy`;
/// bombs and red boxes blow up, the player's car wrecks, anything else
/// (a simple car too: no wreck, no blast) is gone at once and runs its
/// `ActionOnDestroy`.
#[allow(clippy::type_complexity)]
fn destroy(
    In(run): In<Run<DestroyEntity>>,
    ids: MapIds,
    things: Query<(&Transform, Option<&Explosive>, Has<Health>, Has<LocalPlayer>, Option<&OnDestroy>, Option<&MapId>)>,
    mut commands: Commands,
    mut explosions: MessageWriter<Explosion>,
    mut destroyed: MessageWriter<Destroyed>,
    mut run_after: MessageWriter<RunAction>,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((t, explosive, alive, player, on_destroy, id)) = things.get(e) else {
        return false;
    };
    if alive && player {
        destroyed.write(Destroyed(e));
        return true;
    }
    if let Some(explosive) = explosive {
        explosions.write(Explosion {
            at: t.translation.truncate(),
            blast: explosive.blast,
            owner: None,
        });
    }
    if let Some(action) = destroy_action(on_destroy, id) {
        run_after.write(action);
    }
    commands.entity(e).despawn();
    true
}

/// `ApplyImpulseActionData`, `Impulse(id, x, y)`: a change of speed (C#
/// applied `mass × Impulse`), plus up to `RandomImpulse` of it at random.
fn impulse(
    In(run): In<Run<ApplyImpulse>>,
    ids: MapIds,
    mut bodies: Query<(&mut LinearVelocity, &RigidBody)>,
    mut rng: Local<Option<SmallRng>>,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((mut v, RigidBody::Dynamic)) = bodies.get_mut(e) else {
        return false;
    };
    let (i, r) = (run.data.impulse, run.data.random_impulse);
    let k = rng
        .get_or_insert_with(|| SmallRng::seed_from_u64(17))
        .random::<f32>();
    // C# y points down.
    v.0 += Vec2::new(i.x + r.x * k, -(i.y + r.y * k));
    true
}

/// `DamageEntity`, `Damage(id, n)`: negative heals; the blow comes from
/// `Direction` (C# degrees).
fn damage(
    In(run): In<Run<DamageEntity>>,
    ids: MapIds,
    mut health: Query<&mut Health>,
    mut hits: MessageWriter<Hit>,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if run.data.damage >= 0 {
        // C#'s angle was -Direction in its y-down space: +Direction here.
        hits.write(Hit {
            target: e,
            damage: run.data.damage as i32,
            from: run.data.direction.to_radians(),
            by: None,
        });
    } else if let Ok(mut h) = health.get_mut(e) {
        h.hp = (h.hp - run.data.damage as i32).min(h.max);
    }
    true
}
