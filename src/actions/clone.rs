//! Copies of map entities (`ActionRunner.RunCloneObjectBase`):
//! `CloneObject` (template `Clone(id)`) and `CloneObjectByPath`.
//!
//! A clone is the map's entity `ItemId` made again, from its map data, at
//! a new place and angle: at random within `Area` (map cells; around the
//! `RelativeByItem` entity when `IsUseRelativePosition`, at a random
//! angle), or `Count` of them spread along `Path`, each turned
//! `DeltaAngle` from the way the path goes. Each takes one of the
//! `TargetIds` no entity has (up to `Count`); without any it brings back
//! the entity itself if it is gone (a box blown up). A clone remembers
//! its source ([`CloneOf`]), so triggers, buttons and teleports that list
//! the source take its clones too (`GetSourceCustomId`).
//!
//! Not ported: clones of cars (players and `SimpleCars`), which need the
//! AI.

use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::{CloneObject, CloneObjectByPath};
use crate::content::xna::{XRect, XVec2};
use crate::game::{LoadedMap, MapId};
use crate::map::spawn::spawn_entity;
use crate::particles::styles::CLONE_FLASH;
use crate::render::particles::Particles;
use crate::render::sprite::UnitQuad;

/// A clone of the map entity of this id.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloneOf(pub i64);

/// The id an entity answers to in subscriber lists: its source's, if it
/// is a clone.
pub fn source_id(id: &MapId, clone: Option<&CloneOf>) -> i64 {
    clone.map_or(id.0, |c| c.0)
}

pub struct CloneActionsPlugin;

impl Plugin for CloneActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<CloneObject, _>(clone_in_area)
            .add_map_action::<CloneObjectByPath, _>(clone_along_path);
    }
}

/// Where and how a clone stands: world point and angle.
type Place = (Vec2, f32);

#[derive(bevy::ecs::system::SystemParam)]
struct Cloning<'w, 's> {
    ids: MapIds<'w, 's>,
    places: Query<'w, 's, &'static Transform>,
    taken: Query<'w, 's, &'static MapId>,
    map: Res<'w, LoadedMap>,
    quad: Res<'w, UnitQuad>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    particles: ResMut<'w, Particles>,
    commands: Commands<'w, 's>,
    rng: Local<'s, Option<SmallRng>>,
}

impl Cloning<'_, '_> {
    fn rng(&mut self) -> &mut SmallRng {
        self.rng.get_or_insert_with(|| SmallRng::seed_from_u64(41))
    }

    /// `RunCloneObjectBase`: the ids to make, then each at its place.
    fn clone(&mut self, d: &CloneObject, actor: Option<i64>, mut place: impl FnMut(&mut Self, usize) -> Place) {
        let Some(source) = super::target_id(d.item_id, actor) else {
            return;
        };
        let targets: Vec<i64> = if d.target_ids.is_empty() {
            // The entity itself, if it is gone.
            if self.ids.entity(Some(source)).is_some() { vec![] } else { vec![source] }
        } else {
            let taken: Vec<i64> = self.taken.iter().map(|id| id.0).collect();
            d.target_ids.iter().copied().filter(|id| !taken.contains(id)).take(d.count.max(0) as usize).collect()
        };
        if targets.is_empty() {
            return;
        }
        let map = &self.map;
        let Some(data) = map.data.entities.iter().find(|e| e.id == source).cloned() else {
            let car = map.data.players.items.iter().any(|p| p.id == source)
                || map.data.simple_cars.items.iter().any(|c| c.id == source);
            if car {
                debug!("clones of car {source} are not ported yet");
            }
            return;
        };
        let around = if d.is_use_relative_position {
            self.ids
                .entity(super::target_id(d.relative_by_item, actor))
                .and_then(|e| self.places.get(e).ok())
                .map_or(Vec2::ZERO, |t| t.translation.truncate())
        } else {
            Vec2::ZERO
        };
        for (i, target) in targets.into_iter().enumerate() {
            let (at, angle) = place(self, i);
            let at = at + around;
            let (map, quad) = (&*self.map, &*self.quad);
            if let Some(e) = spawn_entity(&mut self.commands, map, quad, &mut self.materials, &data, target, at, angle) {
                if target != source {
                    self.commands.entity(e).insert(CloneOf(source));
                }
                self.particles.try_add(&CLONE_FLASH, at, 0.0, 0.0);
            }
        }
    }
}

/// A random point of a rectangle of map cells (`NextVector2Fake(Area)`),
/// as a world offset, and a random angle.
fn in_area(c: &mut Cloning, a: XRect) -> Place {
    let rng = c.rng();
    let (u, v, turn) = (rng.random::<f32>(), rng.random::<f32>(), rng.random::<f32>());
    let at = c.map.world(a.x as f32 + u * a.w as f32, a.y as f32 + v * a.h as f32);
    (at, turn * std::f32::consts::TAU)
}

fn clone_in_area(In(run): In<Run<CloneObject>>, mut c: Cloning) -> bool {
    let d = run.data;
    c.clone(&d, run.actor, |c, _| in_area(c, d.area));
    true
}

fn clone_along_path(In(run): In<Run<CloneObjectByPath>>, mut c: Cloning) -> bool {
    let d = &run.data;
    let path: Vec<Vec2> = d.path.iter().map(|p: &XVec2| c.map.world(p.x, p.y)).collect();
    let places = along(&path, d.count.max(0) as usize, d.delta_angle.to_radians());
    let base = CloneObject {
        area: d.area,
        item_id: d.item_id,
        target_ids: d.target_ids.clone(),
        count: d.count,
        is_use_relative_position: d.is_use_relative_position,
        relative_by_item: d.relative_by_item,
    };
    c.clone(&base, run.actor, |_, i| places.get(i).copied().unwrap_or_default());
    true
}

/// `RunCloneObjectByPath`: `count` places a path's length / `count` apart,
/// the first one step in, each turned `turn` from the way the path goes
/// there (C# turned `-DeltaAngle` in its y-down space: `+` here).
pub fn along(path: &[Vec2], count: usize, turn: f32) -> Vec<Place> {
    let length: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    if count == 0 || path.len() < 2 || length <= 0.0 {
        return Vec::new();
    }
    let step = length / count as f32;
    let mut out = Vec::new();
    let mut prev = path[0];
    for k in 1..=count {
        let at = point_at(path, step * k as f32);
        out.push((at, (at - prev).to_angle() + turn));
        prev = at;
    }
    out
}

/// The point `distance` along a path (its end past it).
pub(crate) fn point_at(path: &[Vec2], mut distance: f32) -> Vec2 {
    for w in path.windows(2) {
        let l = w[0].distance(w[1]);
        if distance <= l && l > 0.0 {
            return w[0].lerp(w[1], distance / l);
        }
        distance -= l;
    }
    path[path.len() - 1]
}
