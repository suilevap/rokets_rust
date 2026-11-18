//! `AttachAdditionalView`: a sprite stuck to an entity (`AdditionalSprite`),
//! a sign, an icon over a box, a wheel on a wreck.
//!
//! It is placed on the map (`Position`, `DirectionDeg`) and keeps that
//! place relative to the entity as it stood at map start, moving and
//! turning with it after. On the floor with `Depth` 0 or less, else
//! `Depth` units up (over the entity, seen with parallax).
//!
//! Not ported: animated sprites (none in the maps); `RelativeDepth` adds
//! nothing for the entity's own height.

use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::AttachAdditionalView;
use crate::game::{LoadedMap, MapId};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::map_degrees_to_world;

pub struct ViewActionsPlugin;

impl Plugin for ViewActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AttachAdditionalView, _>(attach_view)
            .add_action_sprite_field::<AttachAdditionalView>("Sprite");
    }
}

/// Where an entity stood at map start: from the map, or where it is.
fn start_pose(map: &LoadedMap, id: i64, now: &Transform) -> (Vec2, f32) {
    let from_map = |x: f32, y: f32, deg: f32| (map.world(x, y), map_degrees_to_world(deg));
    if let Some(e) = map.data.entities.iter().find(|e| e.id == id) {
        return from_map(e.position.x, e.position.y, e.direction_deg);
    }
    if let Some(p) = map.data.players.items.iter().find(|p| p.id == id) {
        return from_map(p.position.x, p.position.y, p.direction_deg);
    }
    (now.translation.truncate(), now.rotation.to_euler(EulerRot::ZYX).0)
}

fn attach_view(
    In(run): In<Run<AttachAdditionalView>>,
    ids: MapIds,
    places: Query<(&Transform, &MapId)>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> bool {
    let d = &run.data;
    let Some(e) = ids.entity(run.target(d.item_id)) else {
        return false;
    };
    let (Ok((now, id)), Some(info)) = (places.get(e), d.sprite.as_deref().and_then(|s| map.sprites.get(s))) else {
        return false;
    };
    let (start, start_angle) = start_pose(&map, id.0, now);
    let at = map.world(d.position.x, d.position.y);
    // `GetLocalPoint`: the place in the entity's frame at map start.
    let local = Vec2::from_angle(-start_angle).rotate(at - start);
    let turn = map_degrees_to_world(d.direction_deg) - start_angle;
    let layer = if d.relative_depth || d.depth > 0.0 { Layer::Coin } else { Layer::GroundDecor };
    let tint = SpriteTint {
        color: d.color.to_f32(),
        lit: true,
    };
    let mut sprite = sprite_bundle(info, d.scale, layer, tint, &quad, &mut materials);
    // `Depth` metres up (world units), as C#'s depth with its parallax.
    sprite.2.translation = local.extend(layer.z() + d.depth.max(0.0));
    sprite.2.rotation = Quat::from_rotation_z(turn);
    commands.entity(e).with_child(sprite);
    true
}
