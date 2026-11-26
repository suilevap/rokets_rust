//! Smart controls' brain: commands in, a car's intent out.
//!
//! New in the port (the C# game had only the player's aim help,
//! [`crate::weapon::aim_help`]). The one-finger *Smart* scheme
//! ([`crate::settings::ControlScheme`]) is built as three layers, so that
//! an AI player can later take the finger's place and pass levels by itself:
//!
//! 1. **Input to commands.** A device says what it wants as
//!    [`PlayerCommand`]s (high level: lock this enemy, let it go; later
//!    grab this box, drop it there, boost, go there), sent as
//!    [`Commanded`] messages. The touch screen and the mouse
//!    ([`crate::controls`]) only report a [`Tapped`] ground point; here
//!    [`resolve_taps`] turns it into a command by what stands there. Nothing
//!    below this layer knows about fingers.
//! 2. **The assist.** The [`Assist`] component on the player's car holds
//!    what the commands left: the target lock, and what is pending. Every
//!    frame [`assist_drive`] turns that state, plus whatever the manual
//!    stick already wrote, into the car's
//!    [`PlayerIntent`](crate::controls::PlayerIntent): the aim, the
//!    trigger, the accurate-shot pulses. It reads only the world and the
//!    car, never an input device, so a test or an AI drives it by writing
//!    [`Commanded`] messages (`tests/assist.rs` does).
//! 3. **An AI player** (not built) emits the same [`Commanded`] messages
//!    instead of a finger, from what it sees of the level.
//!
//! The car drives and fires from the intent as always, so none of the
//! weapons, the gadget or the enemies' controls changed.
//!
//! # What the assist does
//!
//! - **Lock.** [`PlayerCommand::LockTarget`] locks an enemy car (alive,
//!   of another team); the lock stays until the target dies or leaves the
//!   map, goes further than twice the gun's range (a lock is a thing to
//!   drive towards, so it outlives the aim help's view and sight), or
//!   [`PlayerCommand::ClearTarget`].
//! - **Soft target.** With no lock the nearest enemy that the aim help
//!   would take, as far as sight goes (alive, visible for auto aim, on the
//!   screen, within the gun's range, in sight) is the target, and the one
//!   chosen stays until it stops qualifying (no flicker between two nearly
//!   as near).
//! - **Auto-aim** ([`Assist::auto_aim`]): the aim points at the target, led
//!   by its speed for guns whose bullets fly (the Plasma gun); the camera
//!   looks that way as it does for a mouse.
//! - **Auto-fire** ([`Assist::auto_fire`]): the trigger is held while the
//!   target is in range and in sight and the gun points at it (the
//!   enemies' `TargetInFireRange`); a gun that fires only on an accurate
//!   pull (Railgun, rockets, Shotgun) gets a pulse every
//!   [`PULSE_SECS`] (the gun's own reload spaces the shots). Guns with no
//!   barrel to aim (the Tesla, the Zombie) fire when the target is in range
//!   and in sight. The Hammer works by itself: left alone.
//! - Aim help ([`crate::weapon::aim_help`]) still leans the aim a quarter
//!   of the way back to its own candidate; with the aim already on a target
//!   it only helps.
//!
//! Without [`Assist`] (the Classic scheme) none of this runs and the
//! controls behave as before. [`sync_assist`] adds it to, or takes it from,
//! the player's car as the settings say.
//!
//! - **Magnet** ([`magnet`]): [`PlayerCommand::Grab`] and
//!   [`PlayerCommand::DropAt`] become a job that drives the gadget's button
//!   and aim over several frames, with the enemies' own timings
//!   ([`crate::gadget::handling`]); out of reach it waits, shown by its
//!   marker, for the player to drive in range.
//!
//! # Not built yet
//!
//! `Boost`, `GoTo` and the taps for them: see
//! `docs/plans/SMART_CONTROLS.md`.

pub mod magnet;
pub mod tap;
pub mod view;

use avian2d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;

use crate::car::CarRadius;
use crate::car::control::{AutoAimVisible, Team};
use crate::controls::{DeviceSystems, LocalPlayer, NoInput, PlayerIntent, reach};
use crate::explosion::Health;
use crate::gadget::{Linkable, Magnet};
use crate::render::camera::MainCamera;
use crate::settings::{ControlScheme, Settings};
use crate::weapon::aim_help::in_view;
use crate::weapon::model::{Kind, closer_by_chebyshev};
use crate::weapon::Weapon;

/// Seconds between the accurate-shot pulses of a Railgun-like gun; its own
/// reload decides how often it really fires.
pub const PULSE_SECS: f32 = 0.3;
/// A lock outlives the gun's range by this factor.
pub const LOCK_RANGE_FACTOR: f32 = 2.0;

/// What a player (a finger, a test, later an AI) asks of a car, at the
/// level of intentions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerCommand {
    /// Aim and fire at this enemy until told otherwise. Ignored for a car
    /// that is dead, gone or not an enemy.
    LockTarget(Entity),
    /// Let the lock go.
    ClearTarget,
    /// Take this thing (a box, a car, a moving gear: whatever is
    /// [`Linkable`]) on the magnet's rope: drive the gadget's button until
    /// it is linked. Pending while the thing is out of reach. Ignored
    /// without a magnet, while something else is on the rope, or for a thing
    /// the magnet cannot take.
    Grab(Entity),
    /// Let the cargo on the rope go over this point (world). Pending while
    /// the point is out of reach. Ignored with nothing on the rope.
    DropAt(Vec2),
    /// Give up the grab or drop in hand (what is on the rope stays).
    CancelCargo,
}

/// A [`PlayerCommand`] for a car (the one with an [`Assist`]).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Commanded {
    pub car: Entity,
    pub command: PlayerCommand,
}

/// A tap (a touch that was lifted at once, or a mouse click, in the Smart
/// scheme) on this point of the ground, world. The devices write it;
/// [`resolve_taps`] decides what it means.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Tapped {
    pub ground: Vec2,
}

/// The assist on a car: its target lock and what it does with it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Assist {
    /// The enemy the player locked.
    pub lock: Option<Entity>,
    /// The enemy picked for the player while nothing is locked.
    pub soft: Option<Entity>,
    /// Turn the gun on the target (`Settings::auto_aim`).
    pub auto_aim: bool,
    /// Fire at the target (`Settings::auto_fire`).
    pub auto_fire: bool,
    /// The grab or drop with the magnet in hand ([`magnet`]).
    pub cargo: Option<magnet::Handling>,
    /// The gadget is to be cancelled by the next [`magnet::assist_cargo`]
    /// (a job was replaced or given up while the button was in its hand).
    cancel_gadget: bool,
    /// Seconds to the next accurate-shot pulse.
    pulse: f32,
}

impl Default for Assist {
    fn default() -> Self {
        Assist { lock: None, soft: None, auto_aim: true, auto_fire: true, cargo: None, cancel_gadget: false, pulse: 0.0 }
    }
}

impl Assist {
    /// What the car shoots at: the lock, else the soft target.
    pub fn target(&self) -> Option<Entity> {
        self.lock.or(self.soft)
    }
}

/// The assist's systems, to order against.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssistSystems;

/// The assist, without what draws it ([`view::AssistViewPlugin`]).
pub struct AssistPlugin;

impl Plugin for AssistPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Tapped>().add_message::<Commanded>().add_systems(
            PreUpdate,
            (sync_assist, resolve_taps, apply_commands, assist_drive, magnet::assist_cargo)
                .chain()
                .in_set(AssistSystems)
                .after(DeviceSystems)
                .after(bevy::input::InputSystems),
        );
    }
}

/// An enemy car as the assist sees it.
#[derive(Clone, Copy, Debug)]
struct Foe {
    pos: Vec2,
    vel: Vec2,
    radius: f32,
}

type CarItem<'a> = (Entity, &'a Team, &'a Position, &'a AutoAimVisible, &'a CarRadius, &'a Health, Option<&'a LinearVelocity>);
type Cars<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Team, &'static Position, &'static AutoAimVisible, &'static CarRadius, &'static Health, Option<&'static LinearVelocity>),
>;

/// Whether a car is one `team` may lock or aim at.
fn is_enemy(team: &Team, other: &Team, health: &Health) -> bool {
    other.0 != team.0 && health.hp > 0
}

/// Adds the [`Assist`] to the player's car in the Smart scheme and takes
/// it away otherwise; keeps its two options as the settings have them.
/// Without [`Settings`] the scheme is Classic.
pub fn sync_assist(
    mut commands: Commands,
    settings: Option<Res<Settings>>,
    mut players: Query<(Entity, Option<&mut Assist>), With<LocalPlayer>>,
) {
    let smart = settings.as_ref().is_some_and(|s| s.control_scheme.in_effect() == ControlScheme::Smart);
    for (car, assist) in &mut players {
        match (assist, smart) {
            (None, true) => {
                commands.entity(car).insert(assist_from(settings.as_deref()));
            }
            (Some(_), false) => {
                commands.entity(car).remove::<Assist>();
            }
            (Some(mut assist), true) => {
                let wanted = assist_from(settings.as_deref());
                if assist.auto_aim != wanted.auto_aim {
                    assist.auto_aim = wanted.auto_aim;
                }
                if assist.auto_fire != wanted.auto_fire {
                    assist.auto_fire = wanted.auto_fire;
                }
            }
            (None, false) => {}
        }
    }
}

fn assist_from(settings: Option<&Settings>) -> Assist {
    settings.map_or_else(Assist::default, |s| Assist { auto_aim: s.auto_aim, auto_fire: s.auto_fire, ..Assist::default() })
}

/// Layer 1: a tap on the ground is a command by what stands there. The
/// nearest of what is within [`tap::TAP_PICK_RADIUS`] of it wins
/// ([`tap::tap_pick`]; an enemy before a thing of the same distance):
///
/// - an enemy (alive, of another team, visible): `LockTarget`. A live enemy
///   car is never grabbed by a tap, even where the magnet could take it:
///   the finger on a car in a fight means the gun;
/// - with a magnet, a [`Linkable`] thing that is no enemy (a box, an ally, a
///   wreck): `Grab`, unless something is on the rope already (that is a
///   drop, below);
/// - otherwise empty ground: with something on the rope `DropAt` the tap;
///   with none the lock is cleared (`ClearTarget`), and a grab still
///   pending is given up (`CancelCargo`). A tap on a box or the cargo itself
///   while towing is a drop at the tap too.
#[allow(clippy::type_complexity)]
pub fn resolve_taps(
    mut taps: MessageReader<Tapped>,
    mut commands: MessageWriter<Commanded>,
    players: Query<(Entity, &Team, &Assist, Option<&Magnet>), With<LocalPlayer>>,
    cars: Cars,
    linkable: Query<(Entity, &Position, Option<&Team>, Option<&Health>), With<Linkable>>,
) {
    for tap in taps.read() {
        for (car, team, assist, magnet) in &players {
            let towing = magnet.is_some_and(|m| m.towing().is_some());
            let enemies = cars
                .iter()
                .filter(|(_, other, _, visible, _, health, _)| is_enemy(team, other, health) && visible.0)
                .map(|(entity, _, at, ..)| (entity, at.0));
            // What the magnet could take, leaving out the player's own car
            // and every live car of another team.
            let things = linkable
                .iter()
                .filter(|(entity, _, other, health)| {
                    magnet.is_some() && *entity != car && !other.zip(*health).is_some_and(|(o, h)| is_enemy(team, o, h))
                })
                .map(|(entity, at, ..)| (entity, at.0));
            let pick = tap::tap_pick(enemies, things, tap.ground, tap::TAP_PICK_RADIUS);
            let mut write = |command| {
                commands.write(Commanded { car, command });
            };
            match pick {
                tap::Pick::Enemy(enemy) => write(PlayerCommand::LockTarget(enemy)),
                _ if towing => write(PlayerCommand::DropAt(tap.ground)),
                tap::Pick::Thing(thing) => write(PlayerCommand::Grab(thing)),
                tap::Pick::None => {
                    write(PlayerCommand::ClearTarget);
                    if assist.cargo.is_some() {
                        write(PlayerCommand::CancelCargo);
                    }
                }
            }
        }
    }
}

/// Carries out the commands on the cars' assists.
#[allow(clippy::type_complexity)]
pub fn apply_commands(
    mut commands: MessageReader<Commanded>,
    mut assists: Query<(&mut Assist, &Team, Option<&Magnet>)>,
    cars: Query<(&Team, &Health)>,
    linkable: Query<(), With<Linkable>>,
) {
    for Commanded { car, command } in commands.read() {
        let Ok((mut assist, team, magnet)) = assists.get_mut(*car) else {
            continue;
        };
        // A new job takes the place of the old; a button in the assist's
        // hand is cancelled first.
        let set_job = |assist: &mut Assist, job: Option<magnet::Job>| {
            if assist.cargo.is_some_and(|h| h.engaged()) {
                assist.cancel_gadget = true;
            }
            assist.cargo = job.map(magnet::Handling::new);
        };
        match *command {
            PlayerCommand::LockTarget(enemy) => {
                if cars.get(enemy).is_ok_and(|(other, health)| is_enemy(team, other, health)) {
                    debug!("assist: locked {enemy:?}");
                    assist.lock = Some(enemy);
                }
            }
            PlayerCommand::ClearTarget => assist.lock = None,
            PlayerCommand::Grab(thing) => {
                if let Some(m) = magnet
                    && m.towing().is_none()
                    && thing != *car
                    && linkable.contains(thing)
                {
                    debug!("assist: grab {thing:?}");
                    set_job(&mut assist, Some(magnet::Job::Grab(thing)));
                }
            }
            PlayerCommand::DropAt(at) => {
                if magnet.is_some_and(|m| m.towing().is_some()) {
                    debug!("assist: drop at {at:?}");
                    set_job(&mut assist, Some(magnet::Job::Drop(at)));
                }
            }
            PlayerCommand::CancelCargo => set_job(&mut assist, None),
        }
    }
}

/// Where to aim to hit a target that moves at `vel`, from `from`, with
/// bullets that fly at `speed` (0: they arrive at once, no lead).
pub fn lead_point(from: Vec2, target: Vec2, vel: Vec2, speed: f32) -> Vec2 {
    if speed <= 0.0 {
        return target;
    }
    target + vel * (from.distance(target) / speed)
}

/// Whether a gun fires only on an accurate pull (`FireType.ShootAccurate`).
pub fn fires_on_pulse(kind: Kind) -> bool {
    matches!(kind, Kind::Railgun | Kind::Rocket | Kind::Shotgun)
}

/// Layer 2: the assist's state and the stick's intent become the car's
/// aim and trigger.
#[allow(clippy::type_complexity)]
pub fn assist_drive(
    time: Res<Time>,
    terrain: Option<Res<crate::ai::SharedTerrain>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut players: Query<(&mut Assist, &mut PlayerIntent, &Weapon, &Position, &Team, &Health, Has<NoInput>), With<LocalPlayer>>,
    cars: Cars,
) {
    let dt = time.delta_secs();
    let camera = cameras.single().ok();
    let blocked = |a: Vec2, b: Vec2| terrain.as_ref().is_some_and(|t| t.0.line_collides(a, b));
    let foe_of = |entity: Entity| {
        cars.get(entity).ok().map(|(_, _, at, _, radius, _, vel)| Foe {
            pos: at.0,
            vel: vel.map_or(Vec2::ZERO, |v| v.0),
            radius: radius.0,
        })
    };
    for (mut assist, mut intent, gun, position, team, health, off) in &mut players {
        if health.hp <= 0 {
            continue;
        }
        let (from, range) = (position.0, gun.options.range);
        // The lock holds while its car lives and is not far away.
        if let Some(lock) = assist.lock {
            let held = cars.get(lock).is_ok_and(|(_, other, at, _, _, health, _)| {
                is_enemy(team, other, health) && closer_by_chebyshev(from, at.0, range * LOCK_RANGE_FACTOR)
            });
            if !held {
                assist.lock = None;
            }
        }
        // With no lock, what the aim help would take, near and in sight.
        let soft = if assist.lock.is_some() || !(assist.auto_aim || assist.auto_fire) {
            None
        } else {
            let wanted = |car: &CarItem| {
                let (_, other, at, visible, radius, health, _) = *car;
                is_enemy(team, other, health)
                    && visible.0
                    && camera.is_none_or(|(c, t)| in_view(c, t, at.0, radius.0))
                    && closer_by_chebyshev(from, at.0, range)
                    && !blocked(from, at.0)
            };
            let kept = assist.soft.and_then(|e| cars.get(e).ok()).filter(wanted).map(|c| c.0);
            kept.or_else(|| {
                cars.iter()
                    .filter(wanted)
                    .min_by(|a, b| a.2.0.distance_squared(from).total_cmp(&b.2.0.distance_squared(from)))
                    .map(|c| c.0)
            })
        };
        if assist.soft != soft {
            debug!("assist: soft target {:?} -> {:?}", assist.soft, soft);
            assist.soft = soft;
        }
        // The Hammer works by itself, and a car nobody drives takes no help.
        if off || gun.kind.auto_fire() {
            continue;
        }
        let Some(foe) = assist.target().and_then(foe_of) else {
            assist.pulse = 0.0;
            continue;
        };
        debug!("assist: target {:?} at {:?}, gun at {:?}, aim_help={}", assist.target(), foe.pos, gun.at, assist.auto_aim);
        if assist.auto_aim {
            let at = lead_point(gun.at, foe.pos, foe.vel, gun.options.bullet_speed);
            let aim = (at - gun.at).normalize_or_zero();
            if aim != Vec2::ZERO {
                intent.aim_dir = aim;
                if !intent.gadget {
                    intent.look = reach(foe.pos - from);
                }
            }
        }
        let in_reach = closer_by_chebyshev(from, foe.pos, range) && !blocked(from, foe.pos);
        // A gun with a barrel must point at the target; a coil or a wave
        // has none.
        let pointing = !gun.kind.is_complex() || gun.in_fire_range(foe.pos, foe.radius);
        if assist.auto_fire && in_reach && pointing {
            intent.fire = true;
            if fires_on_pulse(gun.kind) {
                assist.pulse -= dt;
                if assist.pulse <= 0.0 {
                    intent.fire_accurate = true;
                    assist.pulse = PULSE_SECS;
                }
            }
        } else {
            assist.pulse = 0.0;
        }
    }
}
