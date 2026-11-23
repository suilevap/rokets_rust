//! The accelerator gadget (`AccelerationGadget`, `Weapons/AccelerationGadget.cs`
//! and `ActiveGadget`; 2013, the Race2 map's `ScoutAccelerator`): a burst of
//! speed along the way the car faces, paid for with the whole energy bar.
//!
//! The car of Race2, Revenge and WinnersRoom has it from the start; Derby,
//! Friend and Trophy hand it over with `SetWeapon`'s `NewGadget`.
//!
//! - **Holding** the gadget button (right mouse, right Shift or `E`, the
//!   gamepad's shoulder, the top right of a touch screen) for a quarter of a
//!   second (`ChargeTimeMs` 250, not the magnet's 200) works it, once for
//!   the hold: let go and press again for the next. A shorter press does
//!   nothing. The aim does not matter, the boost is along the car's
//!   `DirectionVector`; the car keeps driving meanwhile
//!   (`IsInSpecialState` is false).
//! - **Ready** (`_isReady`): the energy bar is whole (`CheckIfAvalaible(1)`,
//!   asked every step; an overloaded bar is not enough). Ready, the boost is
//!   the full one: the bar is spent (`TrySpend(1)`, which then refills
//!   after a second's rest at half a bar a second), the car gets an impulse
//!   of `30 × mass`, a speed of 30 units a second more, and a blast of
//!   range 4, force 5 and 20 damage goes off on it (`CreateExplosion`: the
//!   walls shelter, boxes and enemies are thrown and hurt, the car's own
//!   team is spared; its flash, smoke, sound and the camera's jolt are
//!   [`crate::explosion`]'s). The player's pad shakes the view by a tenth
//!   (`ForceFeedback(rate × 0.1)`).
//! - **Not ready**, the same press fizzles: a quarter of the impulse, no
//!   blast, a small red ping on the car (`CreateBip2`), and a shake of
//!   a fortieth. Either way the gadget is then not ready until the energy
//!   is whole again; when it is, a white ring shrinks onto the car's tail
//!   (`CreateReadyAccelerationEffect`, 0.2 s). A car not held for the
//!   player's devices (`!LocalDevice`) has no energy to spend: its gadget
//!   is always whole again a step later, and every press boosts in full.
//! - **Drawn**: while ready, a blue-violet flash glows at the tail
//!   (`DrawLight`: the `Particles\flash` sprite, twice its size, added over
//!   the world, trembling by 0.05 units); while the button is held past
//!   0.06 s a ring of `UI\GadgetUsageInactive` (white at 0.6) closes onto
//!   the car as the charge fills and, once it has worked, `UI\GadgetUsage`
//!   points towards the aim (`DrawControl`, scale `1 + rate`).
//!   On a touch screen the gadget's zone shows only while it is ready
//!   (`GetTouchView`: `BigCircleAccelerator`; the other state has none).
//! - **The AI**: no behaviour tree works it (only the magnet's `Grab` and
//!   `Drop` use the gadget flags); a car with one that is not the player's
//!   boosts when its [`PlayerIntent::gadget`] is held (a script, the debug
//!   server's `/drive`). `gadget_cancel` ends the activity as C#'s
//!   `Cancel` (`_isActive = false`) did, so a button still held starts it
//!   again at the next step.
//!
//! - **`ApplyGadgetToPosition`** ([`super::apply`]) works it too, once, for a
//!   place within a unit of the car: the same boost, ready or not, with no
//!   charge and no regard for the aim; a held button still works it again.
//!
//! Not ported: the gadget's HUD icon (`UI\GadgetIcons\Accelerator`, drawn
//! beside the health bar; no weapon or gadget icon is drawn yet), the move
//! arrow C# draws while the gadget is active (`DrawMoveControlWhenGadgetIsActive`;
//! the movement arrow is not drawn at all), the gamepad's rumble
//! (`ForceFeedback`'s vibration; the camera's jolt is), the commented-out
//! `Accelerate` sound and flame particle, and `_entity.Alive`'s check
//! of C#'s `Entity` (here a car that died has no [`Accelerator`] or no
//! [`PlayerIntent`] left). C# counted whole milliseconds of each frame
//! (16 of 16.67), so its charge takes a step longer than here at 60 steps
//! a second.

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::GadgetSystems;
use super::apply::ApplyGadget;
use crate::actions::Inactive;
use crate::car::{CarRadius, drive_cars};
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::explosion::{Blast, Explosion};
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::{ACCELERATION_READY, BOMB_BEEP};
use crate::render::camera::CameraShake;
use crate::render::layers::Layer;
use crate::render::particles::{FlashBoost, Particles};
use crate::render::premultiply::Premultiplied;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::px;
use crate::weapon::{PlayerEnergy, Weapon};

/// `ActiveGadget.ChargeTimeMs` of this gadget (250), seconds.
pub const CHARGE: f32 = 0.25;
/// `ActiveGadget._length` of this gadget: how near a target `ApplyGadgetToPosition` works it.
const REACH: f32 = 1.0;
/// `_pressedTimer = 1`: what the first step of a hold counts, seconds.
const FIRST_STEP: f32 = 0.001;
/// `_energyConsumption`: the whole bar.
pub const COST: f32 = 1.0;
/// `_accelerationImpulse` per unit of mass: the speed it adds, units a second.
pub const SPEED: f32 = 30.0;
/// The share of the impulse a press that is not ready still gives.
pub const FIZZLE_RATE: f32 = 0.25;
/// `DamageConst.AcceleratorGadgetRange` (4), the force `CreateExplosion` is
/// given (5) and `DamageConst.AcceleratorGadget` (20).
pub const BLAST: Blast = Blast { range: 4.0, force: 5.0, damage: 20 };
/// `ForceFeedback(Position, rate * 0.1f)`: the camera's jolt of a full boost.
pub const SHAKE: f32 = 0.1;
/// `_pressedTimer > 60`: the ring shows after this many seconds of holding.
const RING_AFTER: f32 = 0.06;
/// `Color.White * 0.6f` (`_drawInterfaceColor`).
const RING_ALPHA: f32 = 0.6;
/// `Color.BlueViolet`.
const GLOW_COLOR: [f32; 3] = [138.0 / 255.0, 43.0 / 255.0, 226.0 / 255.0];
/// `scale` of `DrawLight`: the flash at twice its texture.
const GLOW_SCALE: f32 = 2.0;
/// `RandomHelper.NextVector2Fake() * 0.05f`: how far the glow trembles.
const GLOW_TREMBLE: f32 = 0.05;

const FLASH: &str = "Particles\\flash";
const RING_IDLE: &str = "UI\\GadgetUsageInactive";
const RING_WORKING: &str = "UI\\GadgetUsage";

/// Every texture the accelerator draws with, to load with a map that gives
/// a car one.
pub const TEXTURES: [&str; 3] = [FLASH, RING_IDLE, RING_WORKING];

/// The views of a car's accelerator: the glow at its tail, and the ring
/// while the button is held (before it works, after it).
#[derive(Debug)]
struct Views {
    glow: Entity,
    idle: Entity,
    working: Entity,
}

/// The accelerator gadget of a car.
#[derive(Component, Debug)]
pub struct Accelerator {
    /// Seconds the button has been held (`_pressedTimer`).
    held: f32,
    /// It has worked for this hold (`_isActive`).
    active: bool,
    /// The energy is whole (`_isReady`).
    ready: bool,
    views: Option<Views>,
}

impl Accelerator {
    /// A new one is ready (8fbfa979: "ready at start to avoid strange
    /// effect during map start").
    pub fn new() -> Accelerator {
        Accelerator { held: 0.0, active: false, ready: true, views: None }
    }

    /// The boost would be a full one now (`_isReady`).
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// The button's hold has worked the gadget (`IsActive`).
    pub fn active(&self) -> bool {
        self.active
    }

    /// Seconds the button has been held, up to the charge.
    pub fn charge(&self) -> f32 {
        self.held.min(CHARGE)
    }
}

impl Default for Accelerator {
    fn default() -> Self {
        Accelerator::new()
    }
}

/// The accelerator worked: a boost of `rate` (1 in full, [`FIZZLE_RATE`]
/// when it was not ready) from the car's middle `at`.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Boosted {
    pub car: Entity,
    pub at: Vec2,
    pub rate: f32,
}

/// The energy is whole again: a ring shrinks onto the car's tail `at`.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Ready {
    pub car: Entity,
    pub at: Vec2,
}

/// What the views draw with, a frame at a time: apart from gameplay, or
/// the frame rate would change the fights.
#[derive(Resource)]
struct ViewRng(SmallRng);

/// A view entity of the glow or the ring: its car.
#[derive(Component, Debug)]
struct OfCar(Entity);

pub struct AcceleratorPlugin;

impl Plugin for AcceleratorPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Boosted>()
            .add_message::<Ready>()
            .insert_resource(ViewRng(SmallRng::seed_from_u64(41)))
            .add_systems(
                FixedUpdate,
                (use_accelerators, apply_to_accelerators)
                    .chain()
                    .in_set(GadgetSystems)
                    .before(drive_cars)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (spawn_views, effects, draw_views, remove_views).chain().run_if(in_state(GameState::Playing)),
            );
    }
}

type Car<'a> = (
    Entity,
    &'a mut Accelerator,
    &'a mut PlayerIntent,
    &'a Position,
    &'a Rotation,
    &'a mut LinearVelocity,
    &'a CarRadius,
    Option<&'a mut PlayerEnergy>,
    Has<Weapon>,
);

/// `AccelerationGadget.Update` (ready or not by the energy), then the
/// button: held for the charge it works the gadget once
/// (`PlayerCarControlBase.GadgetControl`, `ActiveGadget.ProcessActivity`,
/// `OnStartActivity`).
#[allow(clippy::type_complexity)]
pub fn use_accelerators(
    time: Res<Time>,
    mut cars: Query<Car, Without<Inactive>>,
    mut explosions: MessageWriter<Explosion>,
    mut boosted: MessageWriter<Boosted>,
    mut ready: MessageWriter<Ready>,
) {
    let dt = time.delta_secs();
    for (car, mut a, mut intent, pos, rot, mut velocity, radius, mut energy, has_gun) in &mut cars {
        let at = pos.0;
        let facing = Vec2::from_angle(rot.as_radians());
        // With a gun, its step keeps the energy.
        if !has_gun && let Some(e) = energy.as_mut() {
            e.0.step(dt);
        }
        // `Update`: the bar's whole, or not. Without an energy store (a car
        // the devices do not drive) it always is.
        let enough = energy.as_mut().is_none_or(|e| e.0.check(COST));
        if a.ready && !enough {
            a.ready = false;
        } else if !a.ready && enough {
            a.ready = true;
            ready.write(Ready { car, at: at - facing * radius.0 });
        }
        // `ActiveGadget.Cancel`.
        if std::mem::take(&mut intent.gadget_cancel) {
            a.active = false;
        }
        if !intent.gadget {
            // `EndActivity`: `OnApplyActivity` is empty.
            a.held = 0.0;
            a.active = false;
            continue;
        }
        // `GadgetControl`: the first step of a hold only starts the timer
        // (1 ms); then it counts, and the step after it has passed the
        // charge works the gadget.
        if a.held == 0.0 {
            a.held = FIRST_STEP;
        } else if a.held <= CHARGE {
            a.held += dt;
        }
        if a.held <= CHARGE || a.active {
            continue;
        }
        a.active = true;
        boost(car, at, facing, &mut a, energy.as_deref_mut(), &mut velocity, &mut explosions, &mut boosted);
    }
}

/// `OnStartActivity`: the boost, in full when the gadget is ready and the
/// energy is paid, a fizzle otherwise. Shared by the button and
/// `ApplyGadgetToPosition`.
#[allow(clippy::too_many_arguments)]
fn boost(
    car: Entity,
    at: Vec2,
    facing: Vec2,
    a: &mut Accelerator,
    energy: Option<&mut PlayerEnergy>,
    velocity: &mut LinearVelocity,
    explosions: &mut MessageWriter<Explosion>,
    boosted: &mut MessageWriter<Boosted>,
) {
    let full = a.ready && energy.is_none_or(|e| e.0.try_spend(COST));
    let rate = if full { 1.0 } else { FIZZLE_RATE };
    if full {
        explosions.write(Explosion { at, blast: BLAST, owner: Some(car) });
    }
    // An impulse of `30 × mass` is a speed of 30 whatever the mass.
    velocity.0 += facing * SPEED * rate;
    a.ready = false;
    boosted.write(Boosted { car, at, rate });
}

/// `ApplyGadgetToPosition` for the accelerator (`ActiveGadget`'s, which
/// it does not override): a target nearer than its `_length` (1 unit)
/// works it, the same as the button does, with no regard for the charge or
/// the aim. `_isActive` is not set, so a held button still works it
/// after its own charge.
#[allow(clippy::type_complexity)]
pub fn apply_to_accelerators(
    mut applied: MessageReader<ApplyGadget>,
    mut cars: Query<(&mut Accelerator, &Position, &Rotation, &mut LinearVelocity, Option<&mut PlayerEnergy>), Without<Inactive>>,
    mut explosions: MessageWriter<Explosion>,
    mut boosted: MessageWriter<Boosted>,
) {
    for apply in applied.read() {
        let Ok((mut a, pos, rot, mut velocity, mut energy)) = cars.get_mut(apply.car) else {
            continue;
        };
        if (apply.at - pos.0).length_squared() >= REACH * REACH {
            continue;
        }
        let facing = Vec2::from_angle(rot.as_radians());
        boost(apply.car, pos.0, facing, &mut a, energy.as_deref_mut(), &mut velocity, &mut explosions, &mut boosted);
    }
}

/// The glow and the ring, once per accelerator.
#[allow(clippy::too_many_arguments)]
fn spawn_views(
    mut commands: Commands,
    mut cars: Query<(Entity, &mut Accelerator)>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (mut images, mut premultiplied): (ResMut<Assets<Image>>, ResMut<Premultiplied>),
) {
    for (car, mut a) in &mut cars {
        if a.views.is_some() {
            continue;
        }
        // Added as XNA added a premultiplied texture (`premultiply`).
        let glow = match map.sprites.get(FLASH) {
            Some(flash) => {
                let material = materials.add(StandardMaterial {
                    base_color_texture: Some(premultiplied.of(&flash.image, &mut images)),
                    alpha_mode: AlphaMode::Add,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    depth_bias: Layer::Glow.depth_bias(),
                    ..default()
                });
                commands
                    .spawn((
                        Name::new("Accelerator glow"),
                        MapEntity,
                        OfCar(car),
                        Mesh3d(quad.0.clone()),
                        MeshMaterial3d(material),
                        Transform::from_xyz(0.0, 0.0, Layer::Glow.z()),
                        Visibility::Hidden,
                    ))
                    .id()
            }
            None => commands.spawn((MapEntity, OfCar(car), Transform::default(), Visibility::Hidden)).id(),
        };
        let plain = SpriteTint { color: [1.0, 1.0, 1.0, RING_ALPHA], lit: false };
        let mut ring = |name: &str| {
            let sprite = map.sprites.get(name).map(|i| sprite_bundle(i, 1.0, Layer::Weapon, plain, &quad, &mut materials));
            match sprite {
                Some(s) => commands.spawn((Name::new("Accelerator ring"), MapEntity, OfCar(car), s, Visibility::Hidden)).id(),
                None => commands.spawn((MapEntity, OfCar(car), Transform::default(), Visibility::Hidden)).id(),
            }
        };
        let (idle, working) = (ring(RING_IDLE), ring(RING_WORKING));
        a.views = Some(Views { glow, idle, working });
    }
}

/// The particles and the jolt of what the gadget did: the ready ring, the
/// fizzle's ping and the shake of the player's view (`ForceFeedback`).
fn effects(
    mut boosted: MessageReader<Boosted>,
    mut ready: MessageReader<Ready>,
    mut particles: ResMut<Particles>,
    mut shake: ResMut<CameraShake>,
    players: Query<(), With<LocalPlayer>>,
) {
    for r in ready.read() {
        particles.try_add(&ACCELERATION_READY, r.at, 0.0, 0.0);
    }
    for b in boosted.read() {
        if b.rate < 1.0 {
            particles.try_add(&BOMB_BEEP, b.at, 0.0, 0.0);
        }
        if players.contains(b.car) {
            shake.add(b.rate * SHAKE);
        }
    }
}

/// `DrawLight` (the flash at the tail while ready) and `DrawControl` (the
/// ring while the button is held).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_views(
    flash_boost: Res<FlashBoost>,
    mut rng: ResMut<ViewRng>,
    map: Res<LoadedMap>,
    cars: Query<(&Accelerator, &Transform, &CarRadius, &PlayerIntent, Has<Inactive>, Has<LocalPlayer>)>,
    mut views: Query<(&mut Transform, &mut Visibility, Option<&MeshMaterial3d<StandardMaterial>>), Without<Accelerator>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size_of = |name: &str| map.sprites.get(name).map_or(Vec2::ZERO, |i| i.size) * px(1.0);
    for (a, car, radius, intent, inactive, local) in &cars {
        let Some(v) = &a.views else {
            continue;
        };
        let at = car.translation.truncate();
        let facing = (car.rotation * Vec3::X).truncate();
        // The glow, trembling.
        if let Ok((mut t, mut vis, material)) = views.get_mut(v.glow) {
            let shown = a.ready && !inactive;
            vis.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
            if shown {
                let tremble = Vec2::new(rng.0.random::<f32>() * 2.0 - 1.0, rng.0.random::<f32>() * 2.0 - 1.0) * GLOW_TREMBLE;
                t.translation = (at - facing * radius.0 + tremble).extend(Layer::Glow.z());
                t.scale = (size_of(FLASH) * GLOW_SCALE).extend(1.0);
                if let Some(mut m) = material.and_then(|m| materials.get_mut(&m.0)) {
                    let c = Color::srgb(GLOW_COLOR[0], GLOW_COLOR[1], GLOW_COLOR[2]).to_linear();
                    let b = flash_boost.0;
                    m.base_color = Color::linear_rgba(c.red * b, c.green * b, c.blue * b, 1.0);
                }
            }
        }
        // The ring: shown from 60 ms of holding, only to the player's eye.
        let shown = local && !inactive && intent.gadget && a.held > RING_AFTER;
        // `rate`: 1 as the hold starts, 0 when it has charged.
        let rate = (1.0 - a.held / CHARGE).max(0.0);
        // `WeaponAxisControl.GetAngle()`: towards the aim (zero: 0).
        let aim = if intent.look == Vec2::ZERO { 0.0 } else { intent.look.to_angle() };
        for (view, name, on, turn) in [(v.idle, RING_IDLE, !a.active, 0.0), (v.working, RING_WORKING, a.active, aim)] {
            let Ok((mut t, mut vis, _)) = views.get_mut(view) else {
                continue;
            };
            let visible = shown && on;
            vis.set_if_neq(if visible { Visibility::Inherited } else { Visibility::Hidden });
            if visible {
                t.translation = at.extend(Layer::Weapon.z());
                t.rotation = Quat::from_rotation_z(turn);
                t.scale = (size_of(name) * (1.0 + rate)).extend(1.0);
            }
        }
    }
}

/// The views of a car that lost the gadget (`SetWeapon`, a wreck) or is
/// gone.
fn remove_views(mut commands: Commands, views: Query<(Entity, &OfCar)>, cars: Query<(), With<Accelerator>>) {
    for (view, of) in &views {
        if !cars.contains(of.0) {
            commands.entity(view).despawn();
        }
    }
}
