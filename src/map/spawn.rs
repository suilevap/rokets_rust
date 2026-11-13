//! Builds the map's entities from their templates (`templates.rs`): a
//! sprite, a physics body, and the components that give them behaviour
//! (`Pickup`, `Booster`, `Spin`, `TintLoop`). Nothing here or later looks
//! at the map's entity type.
//!
//! Bodies follow the C# entities (`Entity`, `PhysicEntity`): a box the
//! size of the sprite, friction 0.8, bounce 0.5, linear damping
//! `0.95 × map friction`, angular damping 0.95.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::boost::Booster;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::map::templates::{BodyKind, BodySpec, EntityTemplate, template_for};
use crate::pickups::Pickup;
use crate::render::animate::TintLoop;
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
    let friction = map.data.friction.unwrap_or(1.0);
    for e in map.data.entities.iter() {
        let Some(template) = template_for(e) else {
            continue;
        };
        let Some(first) = template.sprites.first() else {
            continue;
        };
        let Some(info) = map.sprites.get(&first.name).cloned() else {
            continue;
        };
        let size = info.size * px(1.0) * first.scale;
        let angle = map_degrees_to_world(e.direction_deg);
        let mut entity = commands.spawn((
            Name::new(format!("Map entity {}", e.id)),
            MapEntity,
            Transform::from_translation(map.world(e.position.x, e.position.y).extend(0.0))
                .with_rotation(Quat::from_rotation_z(angle)),
            Visibility::default(),
        ));
        if let Some(body) = template.body {
            insert_body(&mut entity, body, size, friction);
        }
        let baked_shadow = e.try_cast_static_shadow && template.body.is_some_and(|b| b.kind != BodyKind::Dynamic);
        if template.casts_shadow && !baked_shadow {
            entity.insert(crate::render::shadows::ShadowCaster { size });
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
        if let Some(rate) = template.spin {
            entity.insert(Spin(rate));
        }
        spawn_sprites(&mut entity, &template, &map, &quad, &mut materials);
    }
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
