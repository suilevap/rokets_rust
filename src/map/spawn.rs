//! Builds the map's entities from their templates (`templates.rs`): a
//! sprite, a physics body, and the components that give them behaviour
//! (`Pickup`, `Booster`, `Spin`, `TintLoop`, `Explosive`, `Beeps`). Nothing here or later looks
//! at the map's entity type.
//!
//! Bodies follow the C# entities (`Entity`, `PhysicEntity`): a box the
//! size of the sprite, friction 0.8, bounce 0.5, linear damping
//! `0.95 × map friction`, angular damping 0.95.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::boost::Booster;
use crate::content::map::{EntityData, EntityType};
use crate::gadget::{Heavy, Linkable};
use crate::map::templates::HEAVY_BOX_DENSITY;
use crate::explosion::Beeps;
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::map::templates::{BodyKind, BodySpec, EntityTemplate, casts_sprite_shadow, template_for};
use crate::pickups::Pickup;
use crate::render::animate::TintLoop;
use crate::render::shadow::CastsShadow;
use crate::render::spin::Spin;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::{map_degrees_to_world, px};

const LINEAR_DAMPING: f32 = 0.95;
const ANGULAR_DAMPING: f32 = 0.95;

pub struct MapEntitiesPlugin;

impl Plugin for MapEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_map_entities);
    }
}

fn spawn_map_entities(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in map.data.entities.iter() {
        let at = map.world(e.position.x, e.position.y);
        spawn_entity(&mut commands, &map, &quad, &mut materials, e, e.id, at, map_degrees_to_world(e.direction_deg));
    }
}

/// One map entity as `e` describes it, with map id `id`, at a world point
/// and angle (its own place, or a clone's).
#[allow(clippy::too_many_arguments)]
pub fn spawn_entity(
    commands: &mut Commands,
    map: &LoadedMap,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
    e: &EntityData,
    id: i64,
    at: Vec2,
    angle: f32,
) -> Option<Entity> {
    let friction = map.data.friction.unwrap_or(1.0);
    let template = template_for(e)?;
    let first = template.sprites.first()?;
    let info = map.sprites.get(&first.name).cloned()?;
    let size = info.size * px(1.0) * first.scale;
    let mut entity = commands.spawn((
        Name::new(format!("Map entity {id}")),
        MapEntity,
        MapId(id),
        Transform::from_translation(at.extend(0.0)).with_rotation(Quat::from_rotation_z(angle)),
        Visibility::default(),
    ));
    if let Some(body) = template.body {
        insert_body(&mut entity, body, size, friction);
    }
    if let Some(boost) = &template.boost {
        entity.insert(Booster {
            speed: boost.speed,
            flash: boost.flash,
        });
    }
    if template.pickup {
        entity.insert(Pickup {
            radius: size.max_element() / 2.0,
        });
    }
    if casts_sprite_shadow(e, &template) {
        entity.insert(CastsShadow { size });
    }
    if let Some(explosive) = template.explosive {
        entity.insert(explosive);
    }
    if template.beeps {
        // Each bomb its own period, 1–2 s (`1 + NextFloatFake()`).
        entity.insert(Beeps::every(1.0 + (e.id as f32 * 0.618_034).fract()));
    }
    // What the magnet can grab (`LinkablePhysicEntity.CanBeLinked`).
    match e.kind {
        EntityType::Box | EntityType::CoinDynamic => {
            entity.insert(Linkable);
        }
        EntityType::HeavyBox => {
            entity.insert((Linkable, Heavy(HEAVY_BOX_DENSITY)));
        }
        _ => {}
    }
    if let Some(rate) = template.spin {
        entity.insert(Spin(rate));
    }
    spawn_sprites(&mut entity, &template, map, quad, materials);
    Some(entity.id())
}

fn insert_body(entity: &mut EntityCommands, body: BodySpec, size: Vec2, friction: f32) {
    entity.insert(Collider::rectangle(size.x, size.y));
    match body.kind {
        BodyKind::Sensor => {
            entity.insert((RigidBody::Static, Sensor, CollisionEventsEnabled));
            return;
        }
        BodyKind::Static => {
            entity.insert(RigidBody::Static);
        }
        BodyKind::Dynamic => {
            let mut axes = LockedAxes::new();
            if body.lock_x {
                axes = axes.lock_translation_x();
            }
            if body.lock_y {
                axes = axes.lock_translation_y();
            }
            if body.lock_rotation {
                axes = axes.lock_rotation();
            }
            entity.insert((
                RigidBody::Dynamic,
                axes,
                ColliderDensity(body.density),
                LinearDamping(LINEAR_DAMPING * friction),
                AngularDamping(ANGULAR_DAMPING),
                TransformInterpolation,
            ));
        }
    }
    entity.insert((
        Friction::new(0.8).with_combine_rule(CoefficientCombine::GeometricMean),
        Restitution::new(0.5).with_combine_rule(CoefficientCombine::Max),
    ));
}

fn spawn_sprites(
    entity: &mut EntityCommands,
    template: &EntityTemplate,
    map: &LoadedMap,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
) {
    let lit = SpriteTint {
        color: [1.0; 4],
        lit: true,
    };
    entity.with_children(|p| {
        for spec in &template.sprites {
            let Some(info) = map.sprites.get(&spec.name) else {
                continue;
            };
            let mut sprite = sprite_bundle(info, spec.scale, spec.layer, lit, quad, materials);
            sprite.2.translation.x = info.size.x * px(1.0) * spec.scale * spec.ahead;
            let mut child = p.spawn(sprite);
            if let Some((wave, track)) = &spec.tint_loop {
                child.insert(TintLoop {
                    wave: *wave,
                    track: track.clone(),
                });
            }
        }
    });
}
