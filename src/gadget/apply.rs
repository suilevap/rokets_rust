//! `ApplyGadgetToPosition`: a map action that works a car's gadget on a
//! place, as if its owner had aimed there and let go. The maps use it to
//! hitch the truck's magnet to something at the start or when the truck
//! is made, with no player or AI to hold the button:
//!
//! - `Derby` (`TruckGrabTurret`, at the start): player 4004's truck takes
//!   turret 3005 by its middle (a static car, dynamic while towed:
//!   [`super::static_cargo`]).
//! - `Race` (at the start): 4004 takes 1035, one unit ahead of it.
//! - `Escort` (`InitTruck`, when the truck is made): the actor takes 1004,
//!   one unit ahead of it; `Escort2` (`seqTruckCreate`): 4002 takes 1010,
//!   one unit behind it.
//! - `GarbageCollector` (a trigger): 4002 takes 3001; `TheCity` (`Brief`,
//!   at the start): 4005 takes 1017 by its middle; `JunkYard` and
//!   `MyGalaxy` the same with a bit of offset.
//!
//! All of them are `Link` (the magnet); no map aims the accelerator this
//! way, but both ported gadgets answer it.
//!
//! C# origin: `ActionRunner.ApplyGadget`, `ActiveGadget.ApplyGadgetToPosition`
//! (the default of every gadget: if the place is nearer than the gadget's
//! `_length`, run its `OnStartActivity`, `OnUpdateActivity` and
//! `OnApplyActivity` with `_isOnActiveDistance` set), `ChainGadget`'s
//! override (lay the rope on the line first), `AccelerationGadget`
//! ([`super::accelerator`]), `Chain.LinkTo`.
//!
//! The action (`ActionRunner.ApplyGadget`; the owner is `GadgetOwnerId`, an
//! item id as every action's: 0 or less is the entity that set it off):
//!
//! - **Owner**: not a car with a gadget the port has (no such id, no
//!   gadget, a gadget of another kind): nothing; the action still counts
//!   as run (C#'s handler returns nothing, so `true`).
//! - **The place**: `RelativePosition` is a point in the target body's own
//!   frame (`TargetItemId`: x ahead, y to the *right*, C# having y down;
//!   world units), turned into the world with the target's position and
//!   rotation (`PhysicBody.GetWorldPoint`). With no such target the point
//!   is a world point itself, in C#'s axes, so `y` is negated
//!   ([`crate::units`]).
//!
//! The place becomes an [`ApplyGadget`] message that each gadget answers in
//! its own step (`FixedUpdate`, the step after the action, before the cars
//! drive): the magnet here, the accelerator in [`super::accelerator`].
//!
//! The magnet:
//!
//! 1. The rope is laid on the line from the car to the place, whether or
//!    not it can reach (`SetElementsPositionByLine`). C#'s integer
//!    division leaves all its points on the car and only the last on the
//!    place ([`super::rope::Rope::lay_by_c_sharp_line`]), so when the rope
//!    takes something its first step yanks it towards the car, about 2.6
//!    units of rope times 256 on the very first step: a box 4 units off
//!    ends against the truck's back, as in C#.
//! 2. A place `REACH` (8, `ActiveDistance`) or more from the car's middle
//!    does nothing more (the base class's `dSq < _length²`).
//! 3. `ChainStart` sounds on the car (`OnStartActivity`).
//! 4. If something is on the rope already (`_currentCargo`) it is let go:
//!    its speed is zeroed (`Cancel`: `ResetDynamics`), the rope breaks
//!    (`ChainBrake`, if it was not already gone with its cargo) and
//!    `ChainStart` sounds again; nothing new is taken.
//! 5. Otherwise a ray from the car's middle towards the place, `REACH` long
//!    (`FindNewCargo`: `Min(_length, MaxLength)`, whatever the distance to
//!    the place), finds what is in the way first, ignoring sensors and
//!    bombs as the button's ray does. A place on the car itself (no
//!    direction) finds nothing.
//! 6. What it finds is taken (`LinkTo`: `ChainConnect`; by its middle; the
//!    rope's pull then works as for the button's grab) if it is [`Linkable`] and no other magnet has it
//!    (`CanBeLinked`, `LinkedMaster == null`). Anything else, or nothing:
//!    the same as 4's end, `ChainStart` once more (`Cancel`, then the
//!    sound).
//!
//! Not ported:
//! - `MoveCargo`'s one drag at the cargo's rope point towards the place
//!   (4), which the very next `Cancel` makes moot but for one step's force.
//! - `Cancel`'s `HighlightLinkableEntitiesEndAll`, the highlight and spark
//!   particles of `FindNewCargo` and `Chain.BrokenEffect`'s particles (the
//!   highlight and sparks are not ported for the button either).
//! - A `_candidate` kept from the button's aiming (C# let a ray that hit
//!   nothing keep it): here the candidate lasts only while the button is
//!   held, so a ray that hits nothing takes nothing.
//! - Where on the cargo the rope holds. C# stepped the hit point inside
//!   the surface by the *previous* ray's normal (`_targetPos`, set before
//!   `_targetNormal` was), which is the zero vector for a gadget's first
//!   ray: the point stayed on the surface, `LinkTo`'s `Fix.TestPoint`
//!   missed it and it held the cargo by its middle (Derby's turret sits
//!   about half a unit further from the truck than the button's grab
//!   would leave it). Here it is always the middle; a second application
//!   by the same gadget, which C# held at a surface point, does the same.
//! - `ActiveGadget._isOnActiveDistance` left set to `true` afterwards
//!   (nothing reads it without the button) and the other gadgets:
//!   `Bomb` and `EditorPointer` are not ported.
//! - The one step between the action and the gadget's answer (C# did it
//!   in the action's own call).

use avian2d::prelude::*;
use bevy::prelude::*;

use super::rope::MAX_LENGTH;
use super::{Heavy, Link, Linkable, Magnet, REACH, attach, unlink};
use crate::actions::{AddMapAction, MapIds, Run};
use crate::content::actions::defs::ApplyGadgetToPosition;
use crate::explosion::Explosive;
use crate::sound::PlaySound;

/// Work the car's gadget on the world point `at`.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct ApplyGadget {
    pub car: Entity,
    pub at: Vec2,
}

pub struct ApplyGadgetPlugin;

impl Plugin for ApplyGadgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ApplyGadget>().add_map_action::<ApplyGadgetToPosition, _>(apply_gadget);
    }
}

/// The action: where, then for whom.
#[allow(clippy::type_complexity)]
fn apply_gadget(
    In(run): In<Run<ApplyGadgetToPosition>>,
    ids: MapIds,
    gadgets: Query<(), super::WithGadget>,
    bodies: Query<(Option<&Position>, Option<&Rotation>, Option<&Transform>)>,
    mut apply: MessageWriter<ApplyGadget>,
) -> bool {
    let data = &run.data;
    let Some(car) = ids.entity(run.target(data.gadget_owner_id)).filter(|&e| gadgets.contains(e)) else {
        return true;
    };
    // C# is y down: the point in the body's frame has y to the right.
    let local = Vec2::new(data.relative_position.x, -data.relative_position.y);
    let at = match ids.entity(run.target(data.target_item_id)).and_then(|e| bodies.get(e).ok()) {
        Some((pos, rot, transform)) => {
            // The physics body's own place; a body with none (a sprite
            // only) by its transform.
            let (pos, angle) = match (pos, rot, transform) {
                (Some(p), Some(r), _) => (p.0, r.as_radians()),
                (_, _, Some(t)) => (t.translation.truncate(), t.rotation.to_euler(EulerRot::ZYX).0),
                _ => (Vec2::ZERO, 0.0),
            };
            pos + Vec2::from_angle(angle).rotate(local)
        }
        None => local,
    };
    apply.write(ApplyGadget { car, at });
    true
}

/// The magnet's answer (see the module's list).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn apply_to_magnets(
    mut applied: MessageReader<ApplyGadget>,
    spatial: SpatialQuery,
    mut magnets: Query<(&mut Magnet, &Position)>,
    linkable: Query<(), With<Linkable>>,
    skipped: Query<(), Or<(With<Sensor>, With<Explosive>)>>,
    mut speeds: Query<(&mut LinearVelocity, &mut AngularVelocity)>,
    mut heavy: Query<(&Heavy, &mut ColliderDensity)>,
    mut play: MessageWriter<PlaySound>,
) {
    for apply in applied.read() {
        let held: Vec<Entity> = magnets.iter().filter_map(|(m, _)| m.towing()).collect();
        let Ok((mut m, pos)) = magnets.get_mut(apply.car) else {
            continue;
        };
        let at = pos.0;
        let mut sound = |name: &'static str| {
            play.write(PlaySound { name, at: Some(at), pitch: 0.0, volume: 1.0 });
        };
        m.rope.lay_by_c_sharp_line(at, apply.at);
        let delta = apply.at - at;
        if delta.length_squared() >= REACH * REACH {
            continue;
        }
        sound("ChainStart");
        if let Some(link) = m.link {
            // `_currentCargo`: let go.
            if let Ok((mut v, mut w)) = speeds.get_mut(link.cargo) {
                v.0 = Vec2::ZERO;
                w.0 = 0.0;
            }
            sound("ChainBrake");
            unlink(&mut m, &mut heavy);
            sound("ChainStart");
            continue;
        }
        let found = Dir2::new(delta).ok().and_then(|dir| {
            let filter = SpatialQueryFilter::from_excluded_entities([apply.car]);
            let hit = spatial.cast_ray_predicate(at, dir, REACH.min(MAX_LENGTH), true, &filter, &|e| !skipped.contains(e))?;
            (linkable.contains(hit.entity) && !held.contains(&hit.entity)).then_some(hit.entity)
        });
        match found {
            Some(cargo) => {
                // By its middle: see "Not ported".
                attach(&mut m, Link { cargo, local: Vec2::ZERO }, &mut heavy);
                sound("ChainConnect");
            }
            None => sound("ChainStart"),
        }
    }
}
