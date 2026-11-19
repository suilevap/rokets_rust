//! Player controls: devices write one intent; the car only reads it.
//!
//! Intent (from `IControl` and `CompositePlayerControl`): the stick says
//! *where to go* in the world, with a strength, not throttle and steering;
//! aim and fire come later with weapons. Each device is its own scheme,
//! and the one touched last is the active one: touch the gamepad and you
//! drive with the gamepad.
//!
//! Schemes here: keyboard (arrows / WASD), gamepad (left stick, d-pad), and
//! touch (a floating stick that starts where the finger lands; with no
//! weapons yet, the whole screen is for driving).
//!
//! Every scheme reads directions on the screen; they are turned into the
//! world by the camera's [`ViewRotation`], so up is up on a portrait phone
//! too, where the view is turned a quarter turn.

use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::render::camera::ViewRotation;

/// What the player wants the car to do, in world space.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct PlayerIntent {
    /// Where to go; length 0..1.
    pub move_dir: Vec2,
    /// Where to aim the gun, world (any length); zero: not aiming
    /// (`WeaponAxisControl`).
    pub aim_dir: Vec2,
    /// The trigger is held (`FireType.ShootMultiple`).
    pub fire: bool,
    /// An accurate shot is asked for (`FireType.ShootAccurate`, what the
    /// Railgun fires on): a pulse on a press, a release or a fresh pull,
    /// held down while the mouse or the gamepad's trigger is. Devices set
    /// it in `PreUpdate` and it stays until the weapon takes it in the
    /// next fixed step, so a pulse between two steps is not lost.
    pub fire_accurate: bool,
    /// Where the player points the gun or the gadget, world, length 0..1
    /// (`WeaponAxisControl`: the cursor over 8 units away is 1); the
    /// camera looks that way (`CameraOffset`).
    pub look: Vec2,
    /// The gadget button is held (`GadgetControl`).
    pub gadget: bool,
    /// The world point the gadget aims at (the mouse); `None`: straight
    /// ahead of the car.
    pub gadget_at: Option<Vec2>,
    /// Or the way it aims, world, length 0..1 of its reach (a touch drag).
    pub gadget_dir: Option<Vec2>,
}

/// Marks the car the local player drives.
#[derive(Component, Debug, Default)]
pub struct LocalPlayer;

/// A car nobody drives for now: the player's, after `SetControl` to
/// `NoneLocal` during an intro (or to an AI not ported yet); a simple
/// car's with a `None*` control (`AINone`, which also keeps a Hammer's saw
/// still).
#[derive(Component, Debug)]
pub struct NoInput;

/// Whether a C# control type is the player's own (`Local`, the hot-seat
/// ones).
pub fn is_players(control: &str) -> bool {
    matches!(control, "Local" | "LocalHotSeat1" | "LocalHotSeat2")
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScheme {
    #[default]
    Keyboard,
    Gamepad,
    Touch,
}

/// The floating touch stick.
#[derive(Resource, Debug, Default)]
pub struct TouchStick {
    pub finger: Option<u64>,
    pub origin: Vec2,
    pub current: Vec2,
}

/// The finger on the gadget's touch zone (`_gadgetTouchIndex`): the top
/// right quarter of the screen, on cars with a gadget.
#[derive(Resource, Debug, Default)]
pub struct GadgetTouch {
    pub finger: Option<u64>,
    pub origin: Vec2,
    pub current: Vec2,
}

/// The finger on the fire zone (`_fireTouchIndex`): the right half of the
/// screen under the gadget's zone (all of it on cars without a gadget),
/// on cars with a gun. Dragging it aims; it fires while it is down.
#[derive(Resource, Debug, Default)]
pub struct FireTouch {
    pub finger: Option<u64>,
    pub origin: Vec2,
    pub current: Vec2,
    /// The aim ignores drags shorter than a fifth of the stick until one
    /// is longer (`_currentWeaponStickDeadZone`).
    pub dead_zone: f32,
}

/// `WeaponStickDeadZone`.
const FIRE_DEAD_ZONE: f32 = 0.2;

/// Whether a screen point (logical pixels) is in the fire zone, for a car
/// with (`gadget`) or without a gadget.
pub fn in_fire_zone(at: Vec2, screen: Vec2, gadget: bool) -> bool {
    at.x > screen.x / 2.0 && (!gadget || at.y >= screen.y / 2.0)
}

/// Whether a screen point (logical pixels) is in the gadget's touch zone.
pub fn in_gadget_zone(at: Vec2, screen: Vec2) -> bool {
    at.x > screen.x / 2.0 && at.y < screen.y / 2.0
}

/// Screen pixels for a full-strength touch stick (56 at 800×480 in C#).
pub const TOUCH_STICK_RADIUS: f32 = 70.0;
const GAMEPAD_DEAD_ZONE: f32 = 0.15;

/// Each scheme's reading this frame (None when it has nothing).
#[derive(Resource, Debug, Default)]
struct SchemeReadings {
    keyboard: Option<Vec2>,
    gamepad: Option<Vec2>,
    touch: Option<Vec2>,
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveScheme>()
            .init_resource::<TouchStick>()
            .init_resource::<GadgetTouch>()
            .init_resource::<FireTouch>()
            .init_resource::<SchemeReadings>()
            .add_systems(
                PreUpdate,
                (read_keyboard, read_gamepad, read_touch, apply_active_scheme, read_gadget, read_weapon)
                    .chain()
                    .after(bevy::input::InputSystems),
            );
    }
}

/// Four keys as a direction on the screen (y up).
fn key_axis(keys: &ButtonInput<KeyCode>, [left, right, up, down]: [KeyCode; 4]) -> Vec2 {
    let k = |key| keys.pressed(key) as i32 as f32;
    Vec2::new(k(right) - k(left), k(up) - k(down))
}

const WASD: [KeyCode; 4] = [KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyW, KeyCode::KeyS];
const ARROWS: [KeyCode; 4] = [KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::ArrowUp, KeyCode::ArrowDown];

/// WASD drives, and the arrows too on a car without a gun; on one with a
/// gun they aim it (`PlayerControlSettings`: `WeaponUp`… are the arrows).
fn read_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    guns: Query<(), (With<LocalPlayer>, With<crate::weapon::Weapon>)>,
    mut readings: ResMut<SchemeReadings>,
    mut active: ResMut<ActiveScheme>,
) {
    let mut v = key_axis(&keys, WASD);
    if guns.is_empty() {
        v = (v + key_axis(&keys, ARROWS)).clamp(Vec2::splat(-1.0), Vec2::ONE);
    }
    let driving_keys = [
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::KeyA,
        KeyCode::KeyD,
        KeyCode::KeyW,
        KeyCode::KeyS,
    ];
    if keys.any_just_pressed(driving_keys) {
        *active = ActiveScheme::Keyboard;
    }
    readings.keyboard = Some(v.normalize_or_zero());
}

fn read_gamepad(
    gamepads: Query<&Gamepad>,
    mut readings: ResMut<SchemeReadings>,
    mut active: ResMut<ActiveScheme>,
) {
    let mut reading = None;
    for pad in &gamepads {
        let mut v = Vec2::new(
            pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0),
            pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0),
        );
        if v.length() < GAMEPAD_DEAD_ZONE {
            v = Vec2::ZERO;
        }
        let dpad = Vec2::new(
            pad.pressed(GamepadButton::DPadRight) as i32 as f32
                - pad.pressed(GamepadButton::DPadLeft) as i32 as f32,
            pad.pressed(GamepadButton::DPadUp) as i32 as f32
                - pad.pressed(GamepadButton::DPadDown) as i32 as f32,
        );
        if dpad != Vec2::ZERO {
            v = dpad.normalize();
        }
        if v != Vec2::ZERO || pad.get_just_pressed().next().is_some() {
            *active = ActiveScheme::Gamepad;
        }
        reading = Some(v.clamp_length_max(1.0));
        if v != Vec2::ZERO {
            break;
        }
    }
    readings.gamepad = reading;
}

#[allow(clippy::too_many_arguments)]
fn read_touch(
    touches: Res<Touches>,
    mut stick: ResMut<TouchStick>,
    mut gadget: ResMut<GadgetTouch>,
    mut fire: ResMut<FireTouch>,
    windows: Query<&Window>,
    with_gadget: Query<(), (With<LocalPlayer>, With<crate::gadget::Magnet>)>,
    with_gun: Query<&crate::weapon::Weapon, With<LocalPlayer>>,
    mut readings: ResMut<SchemeReadings>,
    mut active: ResMut<ActiveScheme>,
) {
    if let Some(id) = fire.finger {
        match touches.get_pressed(id) {
            Some(t) => fire.current = t.position(),
            None => fire.finger = None,
        }
    }
    if let Some(id) = stick.finger
        && touches.get_pressed(id).is_none()
    {
        stick.finger = None;
    }
    if let Some(id) = gadget.finger {
        match touches.get_pressed(id) {
            Some(t) => gadget.current = t.position(),
            None => gadget.finger = None,
        }
    }
    let screen = windows.iter().next().map_or(Vec2::ONE, |w| Vec2::new(w.width(), w.height()));
    for t in touches.iter_just_pressed() {
        *active = ActiveScheme::Touch;
        // A weapon that works by itself has no fire zone.
        let has_gadget = !with_gadget.is_empty();
        let has_gun = with_gun.iter().any(|w| !w.kind.auto_fire());
        if gadget.finger.is_none() && has_gadget && in_gadget_zone(t.position(), screen) {
            gadget.finger = Some(t.id());
            gadget.origin = t.position();
            gadget.current = t.position();
        } else if has_gun && in_fire_zone(t.position(), screen, has_gadget) {
            if fire.finger.is_none() {
                fire.finger = Some(t.id());
                fire.origin = t.position();
                fire.current = t.position();
                fire.dead_zone = FIRE_DEAD_ZONE;
            }
        } else if stick.finger.is_none() {
            stick.finger = Some(t.id());
            stick.origin = t.position();
        }
    }
    readings.touch = stick
        .finger
        .and_then(|id| touches.get_pressed(id))
        .map(|t| {
            stick.current = t.position();
            // Screen y points down; the world's points up.
            let d = (t.position() - stick.origin) / TOUCH_STICK_RADIUS;
            Vec2::new(d.x, -d.y).clamp_length_max(1.0)
        });
    if readings.touch.is_none() && *active == ActiveScheme::Touch {
        readings.touch = Some(Vec2::ZERO);
    }
}

fn apply_active_scheme(
    active: Res<ActiveScheme>,
    readings: Res<SchemeReadings>,
    view: Res<ViewRotation>,
    mut players: Query<(&mut PlayerIntent, Has<NoInput>), With<LocalPlayer>>,
) {
    let v = match *active {
        ActiveScheme::Keyboard => readings.keyboard,
        ActiveScheme::Gamepad => readings.gamepad,
        ActiveScheme::Touch => readings.touch,
    }
    .unwrap_or(Vec2::ZERO);
    let v = view.screen_to_world(v);
    for (mut intent, off) in &mut players {
        intent.move_dir = if off { Vec2::ZERO } else { v };
    }
}

/// The gadget, as `PlayerCarControl` read it: the right mouse button (or
/// the left with Shift) aims at the cursor; right Shift or `E` on the
/// keyboard, or the gamepad's right shoulder, aims ahead of the car, or
/// where the arrows point on a car with a gun (on one without, the
/// arrows steer).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn read_gadget(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::render::camera::MainCamera>>,
    touch: Res<GadgetTouch>,
    view: Res<ViewRotation>,
    mut players: Query<(&mut PlayerIntent, Has<NoInput>, Has<crate::weapon::Weapon>, &Transform), With<LocalPlayer>>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft);
    let by_mouse = mouse.pressed(MouseButton::Right) || (mouse.pressed(MouseButton::Left) && shift);
    let by_key = keys.pressed(KeyCode::ShiftRight)
        || keys.pressed(KeyCode::KeyE)
        || gamepads.iter().any(|p| p.pressed(GamepadButton::RightTrigger));
    // The cursor on the ground plane.
    let cursor = by_mouse
        .then(|| {
            let at = windows.iter().find_map(|w| w.cursor_position())?;
            let (camera, transform) = cameras.iter().next()?;
            let ray = camera.viewport_to_world(transform, at).ok()?;
            let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Z))?;
            Some(ray.get_point(t).truncate())
        })
        .flatten();
    // A touch drag from where the finger landed aims it (`ToGoRelative`):
    // the full reach at a stick's radius.
    let by_touch = touch.finger.is_some();
    let drag = by_touch
        .then(|| {
            let d = (touch.current - touch.origin) / TOUCH_STICK_RADIUS;
            let d = Vec2::new(d.x, -d.y).clamp_length_max(1.0);
            (d.length() > 0.1).then(|| view.screen_to_world(d))
        })
        .flatten();
    let arrows = key_axis(&keys, ARROWS);
    for (mut intent, off, gun, at) in &mut players {
        intent.gadget = !off && (by_mouse || by_key || by_touch);
        intent.gadget_at = cursor;
        intent.gadget_dir = drag.or_else(|| (gun && by_key && arrows != Vec2::ZERO).then(|| view.screen_to_world(arrows.normalize())));
        // `GadgetControl` set the weapon axis to its aim while held.
        intent.look = if !intent.gadget {
            Vec2::ZERO
        } else if let Some(c) = cursor {
            reach(c - at.translation.truncate())
        } else {
            intent.gadget_dir.unwrap_or(Vec2::ZERO)
        };
    }
}

/// The gun, as `PlayerCarControl`, `PlayerCarControlGamePad` and
/// `PlayerCarControlRelativeTouch` read it:
///
/// - the arrows aim (on a car with a gun) and fire while held, but only
///   aim the gadget while its key is down; right Ctrl fires where the
///   gun points; letting the arrows go fires accurately, the way they
///   pointed (`ShootAccurate`; so does a press of right Ctrl);
/// - otherwise the left mouse button fires and the gun follows the
///   cursor, from the first click on, until the arrows take over again;
///   not while the mouse works the gadget;
/// - the gamepad's right trigger fires and its right stick aims;
/// - a finger on the fire zone fires while it is down and aims by how far
///   it is dragged from where it landed; lifting it fires accurately.
///
/// Accurate shots (`ShootAccurate`, [`PlayerIntent::fire_accurate`]): the
/// left mouse button and the gamepad's trigger ask for one every frame
/// they are held, the keyboard and the touch once.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn read_weapon(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::render::camera::MainCamera>>,
    scheme: Res<ActiveScheme>,
    mut touch: ResMut<FireTouch>,
    view: Res<ViewRotation>,
    mut keyboard_only: Local<bool>,
    mut arrows_before: Local<Vec2>,
    mut finger_before: Local<bool>,
    mut players: Query<(&mut PlayerIntent, &crate::weapon::Weapon, Has<NoInput>, Has<crate::gadget::Magnet>), With<LocalPlayer>>,
) {
    let Ok((mut intent, gun, off, has_gadget)) = players.single_mut() else {
        return;
    };
    // `_hasControllableWeapon`: the mouse, the gamepad and the fire zone work
    // a weapon the player aims; one that works by itself (the Hammer) is
    // fired and aimed by the keys only (they still spend its energy).
    let controllable = !gun.kind.auto_fire();
    let mut fire;
    let mut accurate = false;
    let mut aim = Vec2::ZERO;
    // The same as a stick, 0..1 (`WeaponAxisControl`).
    let mut look = Vec2::ZERO;
    let arrows = key_axis(&keys, ARROWS);
    let gadget_key = keys.pressed(KeyCode::ShiftRight) || keys.pressed(KeyCode::KeyE);
    if arrows != Vec2::ZERO {
        // `HandleKeyboadrFireAndGadget`.
        fire = !(has_gadget && gadget_key);
        aim = view.screen_to_world(arrows);
        look = aim;
        *keyboard_only = true;
    } else {
        // `_previousKeyboardWeaponAxis`: let go of the arrows, the shot
        // goes the way they pointed.
        if *arrows_before != Vec2::ZERO {
            accurate = true;
            aim = view.screen_to_world(*arrows_before);
            look = aim;
        }
        fire = keys.pressed(KeyCode::ControlRight);
        accurate |= keys.just_pressed(KeyCode::ControlRight);
        // Browsers turn taps into clicks: on touch the mouse is ignored.
        if controllable && *scheme != ActiveScheme::Touch {
            let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            let gadget_by_mouse = mouse.pressed(MouseButton::Right) || (mouse.pressed(MouseButton::Left) && shift);
            if !(has_gadget && gadget_by_mouse) {
                if mouse.pressed(MouseButton::Left) {
                    fire = true;
                    accurate = true;
                    *keyboard_only = false;
                }
                if !*keyboard_only && let Some(at) = cursor_on_ground(&windows, &cameras) {
                    aim = at - gun.at;
                    look = reach(aim);
                }
            }
        }
    }
    for pad in gamepads.iter().filter(|_| controllable) {
        if pad.pressed(GamepadButton::RightTrigger2) {
            fire = true;
            accurate = true;
        }
        let stick = Vec2::new(
            pad.get(GamepadAxis::RightStickX).unwrap_or(0.0),
            pad.get(GamepadAxis::RightStickY).unwrap_or(0.0),
        );
        if stick.length() > GAMEPAD_DEAD_ZONE && !intent.gadget {
            aim = view.screen_to_world(stick);
            look = aim;
        }
    }
    if touch.finger.is_some() {
        fire = true;
        // `ToGoRelative` with the dead zone, gone once passed.
        let d = (touch.current - touch.origin) / TOUCH_STICK_RADIUS;
        let d = Vec2::new(d.x, -d.y);
        if d.length() >= touch.dead_zone && d != Vec2::ZERO {
            touch.dead_zone = 0.0;
            aim = view.screen_to_world(d.clamp_length_max(1.0));
            look = aim;
        }
    }
    // Letting go of the fire zone.
    accurate |= *finger_before && touch.finger.is_none();
    *finger_before = touch.finger.is_some();
    *arrows_before = arrows;
    intent.fire = !off && fire;
    if !off && accurate {
        intent.fire_accurate = true;
    }
    intent.aim_dir = if off { Vec2::ZERO } else { aim };
    if !intent.gadget {
        intent.look = if off { Vec2::ZERO } else { look };
    }
}

/// `BaseControl.Length`: 256 pixels, the mouse's full reach.
const REACH: f32 = 256.0 / 32.0;

/// `BaseControl.Aim`: a world offset as a stick, full at [`REACH`].
fn reach(v: Vec2) -> Vec2 {
    if v.length() < REACH { v / REACH } else { v.normalize_or_zero() }
}

/// Where the mouse points on the ground plane.
fn cursor_on_ground(
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<crate::render::camera::MainCamera>>,
) -> Option<Vec2> {
    let at = windows.iter().find_map(|w| w.cursor_position())?;
    let (camera, transform) = cameras.iter().next()?;
    let ray = camera.viewport_to_world(transform, at).ok()?;
    let t = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Z))?;
    Some(ray.get_point(t).truncate())
}
