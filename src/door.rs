//! Doors: a box that slides along its facing between a closed and an open
//! place (`Entities/Components/Door.cs`, `AddDoorComponent`,
//! `OpenCloseDoor`).
//!
//! - **`AddDoorComponent`** (usually at map start) makes an entity a door:
//!   it opens its own length × `OpenDistanceScale` ahead along its
//!   facing. `IsStatic` keeps it fixed at both ends, `IsFixOnOpen` once
//!   open; it does not bounce. With `IsDrawHint` a placeholder shows where
//!   it opens to until it does.
//! - **`OpenDoor(id)` / `CloseDoor(id)`** free it and push it that way
//!   (`OpenCloseDoor`); a push can also come from the car.
//! - Reaching either end while moving outwards, it snaps there and stops:
//!   at the open end it is open (`ActionOnOpen` runs once, the hint goes),
//!   back at the start it is closed (`ActionOnClose`, fixed again if
//!   static). Without a door component, `OpenDoor` just pushes the entity.
//!
//! - **Navigation links** (`IsGenerateNavLink`): the door makes a pair of
//!   links in the map's navigation graph across itself (the points are half
//!   its width plus half a cell either side of its start), blocked and far
//!   (`Door.BreakNavigation`: a cell × 32 long) while it is closed; opening
//!   it (`OpenDoor`) makes them as long as it is wide and free, closing it
//!   breaks them again. The graph does not look at `blocked` when it searches
//!   (a way through a closed door is found and refused by the car that wants
//!   it).
//!
//! Not ported: the animated arrow hint, the opening splash. The links follow
//! the open/close *actions* (as C#'s `OpenDoor()` / `CloseDoor()` did), not a
//! door shoved open by a car.

use crate::actions::{AddMapAction, MapIds, Run, RunAction};
use crate::ai::Navigation;
use crate::map::nav::{Blocker, LinkRef};
use crate::content::actions::defs::{AddDoorComponent, OpenCloseDoor};
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use avian2d::prelude::*;
use bevy::prelude::*;

/// `Entities\DoorBoxPlaceholder`: where a door opens to.
const HINT: &str = "Entities\\DoorBoxPlaceholder";

#[derive(Component, Debug)]
pub struct Door {
    start: Vec2,
    open_at: Vec2,
    /// Along the facing, world units.
    length: f32,
    /// `Radius × OpenDistanceScale × 2`: the speed `OpenDoor` gives it.
    push: f32,
    open: bool,
    fix_on_open: bool,
    fixed: bool,
    on_open: Option<String>,
    on_close: Option<String>,
    last_velocity: Vec2,
    hint: Option<Entity>,
    /// The links it made in the navigation graph, and how wide it is
    /// (`Size.Y`) and how far a closed one is (`CellSize * 32`).
    nav_links: Vec<LinkRef>,
    width: f32,
    closed_cost: f32,
}

pub struct DoorPlugin;

impl Plugin for DoorPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AddDoorComponent, _>(add_door)
            .add_action_sprites::<AddDoorComponent>(&[HINT])
            .add_map_action::<OpenCloseDoor, _>(open_close)
            .add_systems(
                FixedUpdate,
                slide_doors.run_if(in_state(GameState::Playing)),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_door(
    In(run): In<Run<AddDoorComponent>>,
    ids: MapIds,
    bodies: Query<(&Position, &Rotation, &Collider, &MapId)>,
    map: Res<LoadedMap>,
    nav: Option<Res<Navigation>>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((pos, rot, collider, _)) = bodies.get(e) else {
        return false;
    };
    let size = box_size(collider);
    let d = &run.data;
    let facing = Vec2::from_angle(rot.as_radians());
    let length = size.x * d.open_distance_scale;
    let open_at = pos.0 + facing * length;
    let hint = (d.is_draw_hint)
        .then(|| map.sprites.get(HINT))
        .flatten()
        .map(|info| {
            let tint = SpriteTint {
                color: [1.0; 4],
                lit: true,
            };
            let mut sprite =
                sprite_bundle(info, 1.0, Layer::GroundDecor, tint, &quad, &mut materials);
            sprite.2.translation = open_at.extend(Layer::GroundDecor.z());
            sprite.2.rotation = Quat::from_rotation_z(rot.as_radians());
            commands
                .spawn((Name::new("Door hint"), MapEntity, sprite))
                .id()
        });
    let name = |s: &Option<String>| s.clone().filter(|s| !s.trim().is_empty());
    let closed_cost = map.cell.0 * 32.0;
    // `Door.Init`: `LinkPoints(start - delta, start + delta, false, this)`, the
    // way across, `delta` being the facing turned a quarter (C#'s
    // `RotateLeft90` in y-down is `(y, -x)` here) by half a width and a half
    // cell; then `BreakNavigation`: the door is closed.
    let nav_links = match (&nav, d.is_generate_nav_link) {
        (Some(nav), true) => {
            let delta = Vec2::new(facing.y, -facing.x) * (size.y + map.cell.0) * 0.5;
            nav.write(|g| {
                let links = g.link_points(pos.0 - delta, pos.0 + delta, false, Some(Blocker(e.to_bits())));
                for link in &links {
                    g.set_link(*link, closed_cost, true);
                }
                links
            })
        }
        _ => Vec::new(),
    };
    if !nav_links.is_empty() {
        debug!("door {:?} made {} navigation links", e, nav_links.len());
    }
    commands.entity(e).insert((
        Door {
            start: pos.0,
            open_at,
            length,
            push: size.max_element() / 2.0 * d.open_distance_scale * 2.0,
            open: false,
            fix_on_open: d.is_fix_on_open,
            fixed: d.is_static,
            on_open: name(&d.action_on_open),
            on_close: name(&d.action_on_close),
            last_velocity: Vec2::ZERO,
            hint,
            nav_links,
            width: size.y,
            closed_cost,
        },
        Restitution::ZERO,
    ));
    if d.is_static {
        commands.entity(e).insert(RigidBody::Static);
    }
    true
}

/// `OpenCloseDoor`, `OpenDoor(id)`, `CloseDoor(id)`.
fn open_close(
    In(run): In<Run<OpenCloseDoor>>,
    ids: MapIds,
    mut bodies: Query<(&Rotation, &mut LinearVelocity, &Collider, Option<&Door>)>,
    nav: Option<Res<Navigation>>,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    let Ok((rot, mut v, collider, door)) = bodies.get_mut(e) else {
        return false;
    };
    let facing = Vec2::from_angle(rot.as_radians());
    let sign = if run.data.open { 1.0 } else { -1.0 };
    match door {
        Some(door) => {
            // `GetImpulse`: mass × facing × push, twice. A fixed door is
            // freed first; its speed goes in with the change of body, as
            // the physics clears a static body's speed.
            let push = facing * door.push * 2.0 * sign;
            if let Some(nav) = &nav {
                // `OpenDoor`: free and as long as the door is wide;
                // `CloseDoor`: `BreakNavigation`.
                let (cost, blocked) = if run.data.open { (door.width, false) } else { (door.closed_cost, true) };
                nav.write(|g| door.nav_links.iter().for_each(|l| g.set_link(*l, cost, blocked)));
            }
            if door.fix_on_open || door.fixed {
                commands
                    .entity(e)
                    .insert((RigidBody::Dynamic, LinearVelocity(push)));
            } else {
                v.0 = push;
            }
        }
        None => {
            // C#'s guess for a plain entity: twice its radius.
            let radius = box_size(collider).max_element() / 2.0;
            v.0 += facing * radius * 2.0 * sign;
        }
    }
    true
}

/// A box body's size (length along its facing, width).
fn box_size(collider: &Collider) -> Vec2 {
    collider.shape_scaled().as_cuboid().map_or(Vec2::ONE, |c| {
        Vec2::new(c.half_extents.x, c.half_extents.y) * 2.0
    })
}

/// `Door.Update`: snap to an end when it gets there moving outwards.
fn slide_doors(
    mut commands: Commands,
    mut doors: Query<(
        Entity,
        &mut Door,
        &mut Position,
        &mut LinearVelocity,
        &Rotation,
        &MapId,
    )>,
    mut run: MessageWriter<RunAction>,
) {
    for (e, mut door, mut pos, mut v, rot, id) in &mut doors {
        let moving = door.last_velocity;
        let mid = (door.start + door.open_at) / 2.0;
        let to_mid = mid - pos.0;
        let at_end = to_mid.length_squared() >= (door.length / 2.0).powi(2);
        if moving != Vec2::ZERO && at_end && moving.dot(to_mid) < 0.0 {
            let facing = Vec2::from_angle(rot.as_radians());
            let action = if moving.dot(facing) > 0.0 {
                pos.0 = door.open_at;
                if door.open {
                    None
                } else {
                    door.open = true;
                    if let Some(hint) = door.hint.take() {
                        commands.entity(hint).despawn();
                    }
                    if door.fix_on_open || door.fixed {
                        commands.entity(e).insert(RigidBody::Static);
                    }
                    door.on_open.clone()
                }
            } else {
                door.open = false;
                pos.0 = door.start;
                if door.fixed {
                    commands.entity(e).insert(RigidBody::Static);
                }
                door.on_close.clone()
            };
            v.0 = Vec2::ZERO;
            if let Some(name) = action {
                run.write(RunAction {
                    name,
                    actor: Some(id.0),
                });
            }
        }
        door.last_velocity = v.0;
    }
}
