//! The actions that work the magnet: [`GrabByChainGadget`] and
//! [`DropByChainGadget`].
//!
//! C# origin: `BTActions/GrabByChainGadget.cs`, `DropByChainGadget.cs`
//! (2014). Both held the gadget's button by `Control.Fire |=
//! GadgetActivityStart`, let go with `GadgetActivityEnd` (which links what the
//! magnet aims at, or drops what it tows) and, ending badly, wrote
//! `Fire = GadgetActivityCancel`. Here an action's tick returns the button
//! ([`Orders::gadget`]) and where it aims ([`Orders::gadget_dir`]), and the
//! cancel is asked of the blackboard ([`AiBlackboard::cancel_gadget`]);
//! `ai::carry_out` writes them into the car's intent, where the magnet
//! (`gadget::use_magnets`) reads them as it reads the player's.
//!
//! The button rules (how long to hold, when to let go) are shared with the
//! Smart scheme's assist: `gadget::handling`.
//!
//! Both read what they work on off the blackboard: the order being carried
//! out ([`AiBlackboard::executing`]: the cargo to grab, the point and radius
//! to drop it at), the car's [`MagnetState`] (what is on the rope, what it
//! would grab) and the [`Thing`](crate::ai::blackboard::Thing)s the gather
//! copies in (where the cargo is, whether it is `Removed`).
//!
//! - [`GrabByChainGadget`]: starts when the car has a magnet and the cargo
//!   is there, within 7 units (`ActiveDistance - 1`) and in sight (no wall:
//!   `LineCollide`); in progress while that holds (without the sight),
//!   the cargo is not yet on the rope and 5 s have not passed. Each step
//!   the button is held while the magnet has nothing it would grab, or the
//!   first second is not over, and let go otherwise (which links the
//!   candidate); the magnet aims at the cargo, a unit way. Ends `Ok` when
//!   the cargo is on the rope; else `Fail`, with the gadget cancelled.
//! - [`DropByChainGadget`]: starts when the magnet tows something and the
//!   drop point is within 7 units and in sight; in progress while it still
//!   tows it (not `Removed`) and the point is within 7. Each step the
//!   button is held while the cargo is further from the point than its own
//!   radius plus the order's, or the first second is not over, but at most
//!   5 s, and let go otherwise (which drops the cargo); the magnet aims at
//!   the point (a part of its reach when nearer). Ends `Ok` when nothing is
//!   on the rope any more and the cargo (not `Removed`) lies within its
//!   radius plus the order's of the point; else `Fail`, the gadget
//!   cancelled.
//!
//! Not ported: C#'s one blackboard `Timer` (`StartTimer`), which these two
//! shared with every `Timer` decorator and `StartTimer` call (the action
//! keeps its own start; no tree restarted it meanwhile); the blackboard's
//! `ActionData` slot, the state of the action instead; `GrabByChainGadget`
//! reading its cargo from a function (`b => cargo`) and `PutCustomData`'s
//! `Cargo`, the order's cargo instead; `GrabByChainGadget.Start` writing
//! `StartTimer` even when it fails (nothing reads it); the aim also
//! written as the gun's (`WeaponAxisControl` was one axis for the gun and
//! the gadget; both are here, which turns a car's gun along it too).
//! `ControlHandle`'s `ClearAxis` after the gadget's end or cancel (the
//! stick set to zero) is done by `carry_out`.

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt::BtAction;

use crate::ai::blackboard::AiBlackboard;
use crate::ai::order_queue::OrderKind;
use crate::ai::orders::{GadgetButton, Orders};
use crate::gadget::REACH;
use crate::gadget::handling::{closer_than, drop_holds, grab_holds};
pub use crate::gadget::handling::{HOLD_MAX, HOLD_MIN, NEAR};
/// `BaseControl.Length` (`ToSimUnits(256)`): how far the control's aim
/// reaches.
const CONTROL_LENGTH: f32 = 8.0;

/// The cargo of the order being carried out (`MoveCargoOrder`'s,
/// `GrabCargoOrder`'s).
pub fn executing_cargo(bb: &AiBlackboard) -> Option<Entity> {
    match bb.executing?.kind {
        OrderKind::MoveCargo { cargo, .. } | OrderKind::GrabCargo { cargo } => Some(cargo),
        _ => None,
    }
}

/// Where the order being carried out drops its cargo, and how near: the
/// order's `Position` and `R`.
pub fn executing_drop(bb: &AiBlackboard) -> Option<(Vec2, f32)> {
    match bb.executing?.kind {
        OrderKind::MoveCargo { to, radius, .. } => Some((to, radius)),
        _ => None,
    }
}

/// `BTActions/GrabByChainGadget`: shoot the magnet at the cargo and take it.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrabByChainGadget;

/// The cargo being grabbed and when the button was first held.
#[derive(Clone, Copy, Debug)]
pub struct Grabbing {
    cargo: Entity,
    since: f64,
}

impl BtAction<AiBlackboard, Orders> for GrabByChainGadget {
    type State = Grabbing;

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<Grabbing> {
        let cargo = executing_cargo(bb)?;
        // `cargo != null && !cargo.Removed`.
        let at = bb.thing(cargo)?.pos;
        (bb.magnet.is_some() && closer_than(at, bb.pos, NEAR) && !bb.line_collides(at, bb.pos))
            .then(|| Grabbing { cargo, since: bb.now() })
    }

    fn is_in_progress(&self, grab: &Grabbing, bb: &AiBlackboard, _: ()) -> bool {
        bb.thing(grab.cargo).is_some_and(|cargo| closer_than(cargo.pos, bb.pos, NEAR))
            && bb.magnet.is_none_or(|m| m.towing != Some(grab.cargo))
            && ((bb.now() - grab.since) as f32) < HOLD_MAX
    }

    fn tick(&self, grab: &mut Grabbing, bb: &mut AiBlackboard, _: ()) -> Orders {
        let (Some(cargo), Some(magnet)) = (bb.thing(grab.cargo), bb.magnet) else {
            return Orders::default();
        };
        let held = (bb.now() - grab.since) as f32;
        let button = if grab_holds(magnet.candidate.is_some(), held) { GadgetButton::Hold } else { GadgetButton::Release };
        // `ToGoWorld(cargo.Position, ScanDistance)`, `SafeNormalize`d.
        let aim = (cargo.pos - bb.pos).normalize_or_zero();
        Orders { gadget: Some(button), gadget_dir: Some(aim), aim_dir: Some(aim), ..Orders::default() }
    }

    fn complete(&self, grab: &mut Grabbing, bb: &mut AiBlackboard, _: ()) -> bool {
        let done = bb.magnet.is_some_and(|m| m.towing == Some(grab.cargo));
        if !done {
            bb.cancel_gadget();
        }
        done
    }
}

/// `BTActions/DropByChainGadget`: hold the cargo over the drop point and let
/// it go there.
#[derive(Clone, Copy, Debug, Default)]
pub struct DropByChainGadget;

/// The cargo being dropped and when the button was first held.
#[derive(Clone, Copy, Debug)]
pub struct Dropping {
    cargo: Entity,
    since: f64,
}

impl BtAction<AiBlackboard, Orders> for DropByChainGadget {
    type State = Dropping;

    fn start(&self, bb: &mut AiBlackboard, _: ()) -> Option<Dropping> {
        let (at, _) = executing_drop(bb)?;
        let cargo = bb.magnet?.towing?;
        (closer_than(at, bb.pos, NEAR) && !bb.line_collides(at, bb.pos)).then(|| Dropping { cargo, since: bb.now() })
    }

    fn is_in_progress(&self, _: &Dropping, bb: &AiBlackboard, _: ()) -> bool {
        let Some((at, _)) = executing_drop(bb) else {
            return false;
        };
        // `LinkedSlave != null && !LinkedSlave.Removed`, and `IsInSpecialState`.
        bb.magnet.and_then(|m| m.towing).is_some_and(|cargo| bb.thing(cargo).is_some()) && closer_than(at, bb.pos, NEAR)
    }

    fn tick(&self, drop: &mut Dropping, bb: &mut AiBlackboard, _: ()) -> Orders {
        let (Some((at, radius)), Some(cargo)) = (executing_drop(bb), bb.magnet.and_then(|m| m.towing).and_then(|c| bb.thing(c))) else {
            return Orders::default();
        };
        let held = (bb.now() - drop.since) as f32;
        let button = if drop_holds(closer_than(at, cargo.pos, cargo.radius + radius), held) {
            GadgetButton::Hold
        } else {
            GadgetButton::Release
        };
        // `ToGoWorld(targetPos, Math.Min(Control.Length, gadget.Length))`: not
        // normalised, so a point nearer than the reach is a part of it.
        let aim = super::to_go_world(bb.pos, at, CONTROL_LENGTH.min(REACH));
        Orders { gadget: Some(button), gadget_dir: Some(aim), aim_dir: Some(aim), ..Orders::default() }
    }

    fn complete(&self, drop: &mut Dropping, bb: &mut AiBlackboard, _: ()) -> bool {
        let done = match executing_drop(bb) {
            Some((at, radius)) => {
                bb.magnet.is_none_or(|m| m.towing.is_none())
                    && bb.thing(drop.cargo).is_some_and(|cargo| closer_than(at, cargo.pos, cargo.radius + radius))
            }
            None => false,
        };
        if !done {
            bb.cancel_gadget();
        }
        done
    }
}
