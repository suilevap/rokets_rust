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
    /// Where to aim; unused until weapons.
    pub aim_dir: Vec2,
    pub fire: bool,
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

/// The player's car, not driven by the player for now (`SetControl` to
/// `NoneLocal` during an intro, or to an AI not ported yet).
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
            .init_resource::<SchemeReadings>()
            .add_systems(
                PreUpdate,
                (read_keyboard, read_gamepad, read_touch, apply_active_scheme, read_gadget)
                    .chain()
                    .after(bevy::input::InputSystems),
            );
    }
}

fn read_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut readings: ResMut<SchemeReadings>,
    mut active: ResMut<ActiveScheme>,
) {
    let pressed = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let mut v = Vec2::ZERO;
    if pressed(KeyCode::ArrowLeft, KeyCode::KeyA) {
        v.x -= 1.0;
    }
    if pressed(KeyCode::ArrowRight, KeyCode::KeyD) {
        v.x += 1.0;
    }
    if pressed(KeyCode::ArrowUp, KeyCode::KeyW) {
        v.y += 1.0;
    }
    if pressed(KeyCode::ArrowDown, KeyCode::KeyS) {
        v.y -= 1.0;
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
    windows: Query<&Window>,
    with_gadget: Query<(), (With<LocalPlayer>, With<crate::gadget::Magnet>)>,
    mut readings: ResMut<SchemeReadings>,
    mut active: ResMut<ActiveScheme>,
) {
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
        if gadget.finger.is_none() && !with_gadget.is_empty() && in_gadget_zone(t.position(), screen) {
            gadget.finger = Some(t.id());
            gadget.origin = t.position();
            gadget.current = t.position();
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
/// keyboard, or the gamepad's right shoulder, aims ahead of the car (the
/// arrows, C#'s gadget aim, steer here).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn read_gadget(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::render::camera::MainCamera>>,
    touch: Res<GadgetTouch>,
    view: Res<ViewRotation>,
    mut players: Query<(&mut PlayerIntent, Has<NoInput>), With<LocalPlayer>>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft);
    let by_mouse = mouse.pressed(MouseButton::Right) || (mouse.pressed(MouseButton::Left) && shift);
    let by_key = keys.pressed(KeyCode::ShiftRight)
        || keys.pressed(KeyCode::KeyE)
        || gamepads.iter().any(|p| p.pressed(GamepadButton::RightTrigger) || p.pressed(GamepadButton::RightTrigger2));
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
    for (mut intent, off) in &mut players {
        intent.gadget = !off && (by_mouse || by_key || by_touch);
        intent.gadget_at = cursor;
        intent.gadget_dir = drag;
    }
}
