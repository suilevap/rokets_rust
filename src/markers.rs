//! Goal markers (`MinimapManager`, `MinimapItemType.Goal`; made by
//! `AddMarker(itemId,markerId)` / `AddMarkerActionData`, removed by
//! `RemoveMarker(markerId)`).
//!
//! A marker points the player's car at a goal: an entity, or else the
//! middle of the trigger of that id. Its arrow (`minimap_goal`) stands
//! just off the car (the car's radius + 40 px) towards the goal, once the
//! goal is further than 128 px, fading in over the next 256 px.

use bevy::prelude::*;

use crate::actions::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{AddMarker, RemoveMarker};
use crate::controls::LocalPlayer;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::camera::CameraTarget;
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

const ARROW: &str = "Minimap\\minimap_goal";
/// `ScanLength`: nearer than this, no arrow.
const SCAN: f32 = 128.0 / 32.0;
/// `ScanLengthBig`: the arrow fades in over this.
const FADE: f32 = 256.0 / 32.0;
/// `DrawRadius`: how far out from the car's edge.
const OUT: f32 = 40.0 / 32.0;

/// What a marker points at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    Entity(Entity),
    Point(Vec2),
}

/// A marker's arrow, by its id.
#[derive(Component, Debug)]
pub struct Marker {
    pub id: i64,
    pub goal: Goal,
}

pub struct MarkersPlugin;

impl Plugin for MarkersPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AddMarker, _>(add_marker)
            .add_map_action::<RemoveMarker, _>(remove_marker)
            .add_action_sprites::<AddMarker>(&[ARROW])
            .add_systems(PostUpdate, point_markers.before(TransformSystems::Propagate).run_if(in_state(GameState::Playing)));
    }
}

/// The arrow's place and turn around a car at `car` (radius `radius`),
/// and how strongly it shows; `None` near the goal.
pub fn arrow(car: Vec2, radius: f32, goal: Vec2) -> Option<(Vec2, f32, f32)> {
    let to = goal - car;
    let length = to.length();
    if length <= SCAN {
        return None;
    }
    let dir = to / length;
    Some((car + dir * (radius + OUT), dir.to_angle(), ((length - SCAN) / FADE).min(1.0)))
}

fn add_marker(
    In(run): In<Run<AddMarker>>,
    ids: MapIds,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> bool {
    let d = &run.data;
    let goal = match ids.entity(run.target(d.item_id)) {
        Some(e) => Goal::Entity(e),
        None => {
            let Some(t) = map.data.triggers.iter().find(|t| t.id == d.item_id) else {
                return false;
            };
            let a = t.area;
            Goal::Point(map.world(a.x as f32 + a.w as f32 / 2.0, a.y as f32 + a.h as f32 / 2.0))
        }
    };
    let Some(info) = map.sprites.get(ARROW) else {
        return false;
    };
    let tint = SpriteTint {
        color: [1.0; 4],
        lit: false,
    };
    let sprite = sprite_bundle(info, 1.0, Layer::Coin, tint, &quad, &mut materials);
    commands.spawn((Name::new(format!("Marker {}", d.marker_id)), MapEntity, Marker { id: d.marker_id, goal }, sprite, Visibility::Hidden));
    true
}

fn remove_marker(In(run): In<Run<RemoveMarker>>, markers: Query<(Entity, &Marker)>, mut commands: Commands) -> bool {
    for (e, m) in &markers {
        if m.id == run.data.marker_id {
            commands.entity(e).despawn();
        }
    }
    true
}

#[allow(clippy::type_complexity)]
fn point_markers(
    cars: Query<(&Transform, &CameraTarget), (With<LocalPlayer>, Without<Marker>)>,
    places: Query<&GlobalTransform, Without<Marker>>,
    mut markers: Query<(&Marker, &mut Transform, &mut Visibility, &mut SpriteTint)>,
) {
    let car = cars.iter().next();
    for (m, mut t, mut visibility, mut tint) in &mut markers {
        let goal = match m.goal {
            Goal::Point(p) => Some(p),
            Goal::Entity(e) => places.get(e).ok().map(|g| g.translation().truncate()),
        };
        let shown = car.zip(goal).and_then(|((ct, target), goal)| arrow(ct.translation.truncate(), target.radius, goal));
        match shown {
            Some((at, angle, alpha)) => {
                t.translation = at.extend(Layer::Coin.z());
                t.rotation = Quat::from_rotation_z(angle);
                tint.color[3] = alpha;
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
}
