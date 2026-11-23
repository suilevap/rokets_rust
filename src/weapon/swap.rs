//! `SetWeapon`: a map action that hands a car another gun or gadget while
//! the map runs (the player's Railgun in `Revenge`, the Plasma gun in
//! `TestLab`, the clones' `MinigunSlowInacurate`, a Hammer for an
//! `ActionOnCreate`, the magnet in `Teleport`).
//!
//! C# origin: `ActionRunner.SetWeaponRun`, `ControlableEntity.Weapon`'s
//! setter (the old gun is `Destroy`ed, the new one made by
//! `WeaponFactory.CreateObject`, the local player's aim preferences set),
//! `Car.Gadget`'s setter, `GadgetFactory`, the `SetWeapon(weapon)` template.
//!
//! What the action does, by `ItemId` (0 or less: the entity that set it
//! off, as for every action):
//!
//! - **Not a car** (no such id, or nothing that can carry a gun): nothing.
//!   A wreck has none either (no [`Health`]; C#'s entity was gone).
//! - **`NewWeapon`**: `Default` leaves the car's gun alone. `None` takes it
//!   away (the factory answers null). Any other type makes a new
//!   [`Weapon`], from the car's [`Mounts`], with the preset the car's
//!   control is held for ([`LocalDevice`]: the player's, as
//!   `WeaponFactory.RegisterObject` picks by `Control.LocalDevice`) and
//!   replaces the old one: its reload, aim, recoil, beam, saw and the like
//!   start over, and its views go with it (each view system follows the
//!   [`Weapon`], so the old sprite, discs, beam parts or pull line are
//!   dropped and the new kind's drawn). The player's car keeps its energy
//!   store (it has one even if it began with no gun) and a simple car that
//!   had no gun gets the sounds one needs. AI cars need nothing: the AI
//!   reads the car's [`Weapon`] every step (range, whether its shot is
//!   immediate, whether it is ready), so its attack style follows the gun.
//! - **`NewGadget`** (a `Car` only: the player's and the other `Players`):
//!   `Default` leaves it. `Link` gives the car the magnet, at its
//!   `GadgetPosition`, unless it has one, and takes the accelerator
//!   away. `Accelerator` gives it the accelerator (ready, as new), unless
//!   it has one, and takes the magnet away; the player's car gets an
//!   energy store for it if it had none, and nothing is shown or played
//!   (C# only announced the magnet). `None` takes either away.
//! - **Not silently** (`ChangeSilently` false), and only for a car held for
//!   the player's devices: a new gun (not `None`) plays `WeaponReload` and
//!   shows a ring of `UI\BigCircleGun` in the middle of the screen
//!   ([`WeaponCollected`], drawn by [`super::collected`]); the magnet plays
//!   `ChainBrake` with a ring of `UI\BigCircleGadget`.
//!
//! Not ported:
//! - The other gadgets (`Bomb`, `EditorPointer`): the car keeps the gadget
//!   it has, neither exists yet.
//! - A gadget the car already has, given again, is not made anew (C#
//!   built a new one: an accelerator that is ready again whatever the
//!   energy, a magnet with no rope).
//! - `AdjustWeaponMaxAngle` is not run again for a static car given a gun
//!   (C# ran it only when the car was made, so neither does this).
//! - A gun made at the car's first update when its hand is empty
//!   (`ControlableEntity`'s one-off refill from `Cars.xml`); a car that
//!   [`crate::car::spawn_car`] made with no gun stays with none until
//!   this action gives it one.
//! - The magnet taken from a car in the middle of towing drops what it
//!   tows with no word (C# kept the old gadget's chain until it was
//!   collected).
//! - A `Weapon` setter's `Control.Init(this)`: it only told the player's
//!   control whether there is a gun to fire by hand and a gadget (here the
//!   controls look at the car every step).

use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use super::model::for_type;
use super::{PlayerEnergy, Weapon, has_aim_line, textures_of};
use crate::actions::{AddMapAction, MapIds, Run};
use crate::car::control::LocalDevice;
use crate::car::{Car, Mounts};
use crate::content::actions::TemplateArgs;
use crate::content::actions::defs::{GadgetType, SetWeapon, WeaponType};
use crate::content::node::Node;
use crate::controls::LocalPlayer;
use crate::explosion::Health;
use crate::gadget::Magnet;
use crate::gadget::accelerator::Accelerator;
use crate::sound::{CarSounds, PlaySound};

/// The player's car was handed something, not silently: a ring appears
/// in the middle of the screen (`CreateWeaponCollectedUI`,
/// `CreateMagnetCollectedUI`).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponCollected {
    /// The magnet, not a gun.
    pub gadget: bool,
}

/// What the action looks at of a car.
#[derive(QueryData)]
struct Target {
    mounts: &'static Mounts,
    position: Option<&'static avian2d::prelude::Position>,
    rotation: Option<&'static avian2d::prelude::Rotation>,
    local_device: Has<LocalDevice>,
    local_player: Has<LocalPlayer>,
    car: Has<Car>,
    energy: Has<PlayerEnergy>,
    sounds: Has<CarSounds>,
    magnet: Option<&'static Magnet>,
    accelerator: Has<Accelerator>,
}

pub struct SwapPlugin;

impl Plugin for SwapPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WeaponCollected>()
            .add_map_action::<SetWeapon, _>(set_weapon)
            .add_action_sprites_by::<SetWeapon>(sprites_of_action)
            .add_template_sprites("SetWeapon", sprites_of_template);
    }
}

#[allow(clippy::type_complexity)]
fn set_weapon(
    In(run): In<Run<SetWeapon>>,
    ids: MapIds,
    cars: Query<Target, With<Health>>,
    mut commands: Commands,
    mut play: MessageWriter<PlaySound>,
    mut collected: MessageWriter<WeaponCollected>,
) -> bool {
    let id = run.target(run.data.item_id);
    let Some(e) = ids.entity(id) else {
        return false;
    };
    // `GetEntity(..) as ControlableEntity`.
    let Ok(car) = cars.get(e) else {
        return true;
    };
    let data = &run.data;
    let silent = data.change_silently;

    if data.new_weapon != WeaponType::Default {
        let name = WeaponType::NAMES[data.new_weapon as usize];
        match for_type(name, car.local_device) {
            Some((kind, options)) => {
                let mut rng = SmallRng::seed_from_u64(id.unwrap_or(0) as u64);
                let mut gun = Weapon::new(kind, options, car.mounts.weapon, car.mounts.size, car.local_device, &mut rng);
                // Where it is now, so nothing reads it at the origin until
                // the next step places it with the cabin.
                if let (Some(at), Some(angle)) = (car.position, car.rotation) {
                    gun.place(at.0, angle.as_radians(), (Vec2::ZERO, 0.0));
                }
                commands.entity(e).insert(gun);
                // `EnergyComponent` is every entity's; ours is the player's.
                if car.local_player && !car.energy {
                    commands.entity(e).insert(PlayerEnergy::default());
                }
                // The gun's loop (a Minigun's) sounds on a simple car.
                if !car.sounds {
                    commands.entity(e).insert(CarSounds::new(None, 0.0));
                }
            }
            // `None`: the factory answers null.
            None => {
                commands.entity(e).remove::<Weapon>();
            }
        }
        if !silent && car.local_device && data.new_weapon != WeaponType::None {
            play.write(PlaySound { name: "WeaponReload", at: None, pitch: 0.0, volume: 1.0 });
            collected.write(WeaponCollected { gadget: false });
        }
    }

    if car.car {
        match data.new_gadget {
            GadgetType::Link => {
                if car.magnet.is_none() {
                    commands.entity(e).insert(Magnet::new(car.mounts.gadget));
                }
                // One gadget a car: the accelerator's views go with it.
                if car.accelerator {
                    commands.entity(e).remove::<Accelerator>();
                }
                if !silent && car.local_device {
                    play.write(PlaySound { name: "ChainBrake", at: None, pitch: 0.0, volume: 1.0 });
                    collected.write(WeaponCollected { gadget: true });
                }
            }
            GadgetType::Accelerator => {
                if !car.accelerator {
                    commands.entity(e).insert(Accelerator::new());
                }
                if let Some(magnet) = car.magnet {
                    for view in magnet.view_entities() {
                        commands.entity(view).despawn();
                    }
                    commands.entity(e).remove::<Magnet>();
                }
                // `EnergyComponent` is every entity's; ours is the player's.
                if car.local_player && !car.energy {
                    commands.entity(e).insert(PlayerEnergy::default());
                }
            }
            GadgetType::None => {
                if let Some(magnet) = car.magnet {
                    for view in magnet.view_entities() {
                        commands.entity(view).despawn();
                    }
                    commands.entity(e).remove::<Magnet>();
                }
                if car.accelerator {
                    commands.entity(e).remove::<Accelerator>();
                }
            }
            // `Default` keeps what the car has; the others are not ported.
            GadgetType::Default | GadgetType::Bomb | GadgetType::EditorPointer => {}
        }
    }
    true
}

/// The textures a gun and a gadget need, with the ring of the effect
/// (loaded with the map, for the car that is handed them later).
fn sprites(weapon: WeaponType, gadget: GadgetType, silent: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if weapon != WeaponType::Default
        && let Some((kind, _)) = for_type(WeaponType::NAMES[weapon as usize], true)
    {
        out.extend(textures_of(kind).into_iter().map(String::from));
        if has_aim_line(kind) {
            out.extend(super::AIM_LINE_TEXTURES.map(String::from));
        }
        if !silent {
            out.push(super::collected::GUN_RING.to_string());
        }
    }
    if gadget == GadgetType::Link {
        out.extend(crate::gadget::TEXTURES.map(String::from));
        if !silent {
            out.push(super::collected::GADGET_RING.to_string());
        }
    }
    if gadget == GadgetType::Accelerator {
        out.extend(crate::gadget::accelerator::TEXTURES.map(String::from));
    }
    out
}

fn sprites_of_action(node: &Node) -> Vec<String> {
    let read = |field: &str| node.child(field).map(|n| n.text.trim().to_string());
    let weapon = read("NewWeapon").and_then(|t| WeaponType::from_name(&t)).unwrap_or(WeaponType::Default);
    let gadget = read("NewGadget").and_then(|t| GadgetType::from_name(&t)).unwrap_or(GadgetType::Default);
    let silent = read("ChangeSilently").is_some_and(|t| t.eq_ignore_ascii_case("true"));
    sprites(weapon, gadget, silent)
}

fn sprites_of_template(args: &TemplateArgs) -> Vec<String> {
    let weapon = args.text(0).and_then(WeaponType::from_name).unwrap_or(WeaponType::Default);
    sprites(weapon, GadgetType::Default, false)
}
