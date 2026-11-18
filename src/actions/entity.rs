//! Actions on one entity (`ActionRunner`): activate, destroy, push, hurt.

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::Deserialize;

use super::{AddMapAction, MapAction, MapIds, Run, Templates};
use crate::content::xna::{XVec2, boolean, lenient_int};
use crate::explosion::{Destroyed, Explosion, Explosive, Health, Hit};

/// Out of the game for now (`Entity.Deactivate`): hidden, out of the physics.
#[derive(Component, Debug)]
pub struct Inactive;

pub struct EntityActionsPlugin;

impl Plugin for EntityActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<SetActive, _>(set_active)
            .add_map_action::<Destroy, _>(destroy)
            .add_map_action::<Impulse, _>(impulse)
            .add_map_action::<Damage, _>(damage);
    }
}

/// `ActivateEntityActionData`, `ActivateEntity(id)`, `DeactivateEntity(id)`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SetActive {
    #[serde(default, deserialize_with = "lenient_int")]
    item_id: i64,
    #[serde(default, deserialize_with = "boolean")]
    deactivate: bool,
}

impl MapAction for SetActive {
    const TYPE: &'static str = "ActivateEntityActionData";

    fn templates() -> Templates<Self> {
        vec![
            ("ActivateEntity", |a| Some(SetActive { item_id: a.int(0)?, deactivate: false })),
            ("DeactivateEntity", |a| Some(SetActive { item_id: a.int(0)?, deactivate: true })),
        ]
    }
}

fn set_active(In(run): In<Run<SetActive>>, ids: MapIds, mut commands: Commands) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if run.data.deactivate {
        commands.entity(e).insert((Inactive, RigidBodyDisabled, ColliderDisabled, Visibility::Hidden));
    } else {
        commands
            .entity(e)
            .remove::<(Inactive, RigidBodyDisabled, ColliderDisabled)>()
            .insert(Visibility::Inherited);
    }
    true
}

/// `DestroyEntityActionData`, `DestroyEntity(id)`: `Entity.Destroy`;
/// bombs and red boxes blow up, the player's car wrecks.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Destroy {
    #[serde(default, deserialize_with = "lenient_int")]
    item_id: i64,
}

impl MapAction for Destroy {
    const TYPE: &'static str = "DestroyEntityActionData";

    fn templates() -> Templates<Self> {
        vec![("DestroyEntity", |a| Some(Destroy { item_id: a.int(0)? }))]
    }
}

fn destroy(
    In(run): In<Run<Destroy>>,
    ids: MapIds,
    things: Query<(&Transform, Option<&Explosive>, Has<Health>)>,
    mut commands: Commands,
    mut explosions: MessageWriter<Explosion>,
    mut destroyed: MessageWriter<Destroyed>,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((t, explosive, alive)) = things.get(e) else {
        return false;
    };
    if alive {
        destroyed.write(Destroyed(e));
        return true;
    }
    if let Some(explosive) = explosive {
        explosions.write(Explosion {
            at: t.translation.truncate(),
            blast: explosive.blast,
        });
    }
    commands.entity(e).despawn();
    true
}

/// `ApplyImpulseActionData`, `Impulse(id, x, y)`: a change of speed (C#
/// applied `mass × Impulse`), plus up to `RandomImpulse` of it at random.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Impulse {
    #[serde(default, deserialize_with = "lenient_int")]
    item_id: i64,
    #[serde(default)]
    impulse: XVec2,
    #[serde(default)]
    random_impulse: XVec2,
}

impl MapAction for Impulse {
    const TYPE: &'static str = "ApplyImpulseActionData";

    fn templates() -> Templates<Self> {
        vec![("Impulse", |a| {
            Some(Impulse {
                item_id: a.int(0)?,
                impulse: XVec2 { x: a.num(1)?, y: a.num(2)? },
                random_impulse: XVec2::default(),
            })
        })]
    }
}

fn impulse(
    In(run): In<Run<Impulse>>,
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
    let k = rng.get_or_insert_with(|| SmallRng::seed_from_u64(17)).random::<f32>();
    // C# y points down.
    v.0 += Vec2::new(i.x + r.x * k, -(i.y + r.y * k));
    true
}

/// `DamageEntity`, `Damage(id, n)`: negative heals; the blow comes from
/// `Direction` (C# degrees).
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Damage {
    #[serde(default, deserialize_with = "lenient_int")]
    item_id: i64,
    #[serde(default, deserialize_with = "lenient_int")]
    damage: i64,
    #[serde(default)]
    direction: f32,
}

impl MapAction for Damage {
    const TYPE: &'static str = "DamageEntity";

    fn templates() -> Templates<Self> {
        vec![("Damage", |a| Some(Damage { item_id: a.int(0)?, damage: a.int(1)?, direction: 0.0 }))]
    }
}

fn damage(In(run): In<Run<Damage>>, ids: MapIds, mut health: Query<&mut Health>, mut hits: MessageWriter<Hit>) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    if run.data.damage >= 0 {
        // C#'s angle was -Direction in its y-down space: +Direction here.
        hits.write(Hit {
            target: e,
            damage: run.data.damage as i32,
            from: run.data.direction.to_radians(),
        });
    } else if let Ok(mut h) = health.get_mut(e) {
        h.hp = (h.hp - run.data.damage as i32).min(h.max);
    }
    true
}
