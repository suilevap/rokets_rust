//! The pulsing control pictures of `HighlightController`: the picture of
//! the control the player has to use fades in and out, until another
//! `HighlightController` (or the map's end) turns it off. Bevy UI over the
//! game, in `Update` (screen space, as the splashes); the action
//! ([`crate::actions::ControlHighlight`]) only sets the flags.
//!
//! C# origin: the `DrawUI` of the three player controls, each drawing its
//! own device's pictures when `ControlHelper.Highlight*Control` is set, at
//! the opacity of `PhaseHelper.Triangle_Period500` (0 → 1 → 0 a second):
//! - keyboard and mouse (`PlayerCarControl.DrawUI`): the mouse's right
//!   button top right for the gadget (a car with one), its left button
//!   bottom right for the weapon (one the player fires), the arrow keys
//!   (`KeyboardControlsLeftRight` unless the car is `RestrictX`,
//!   `…UpDown` unless `RestrictY`) at the left for driving;
//! - gamepad (`PlayerCarControlGamePad.DrawUI`): the pad's picture bottom
//!   right (opacity 0.8, not pulsing) with the right bumper (gadget), the
//!   right trigger (weapon) and the stick that works them (both) or the
//!   left stick (driving) pulsing on it;
//! - touch (`PlayerCarControlRelativeTouch.DrawUI`): the round pictures of
//!   the touch zones, which C# drew faintly all the time (opacity 0.14,
//!   0.1 for driving) and pulsing when highlighted: the gadget's top
//!   right (`BigCircleGadget`, or the accelerator's `BigCircleAccelerator`),
//!   the gun's (`BigCircleGun`, red) bottom right, the stick's (`BigCircle`)
//!   bottom left, 1.8 × 56 px of radius at 800 × 480.
//!
//! The device is the one used last ([`ActiveScheme`]; C# picked one per
//! platform), so on a desktop a highlight shows the keyboard's pictures;
//! the touch circles are only to be seen on a phone or tablet (or by
//! touching the page). [`hints`] is the layout as plain data (tested);
//! pictures are laid out for 800 × 480 and grow with the window's height.
//!
//! Branches of the three `DrawUI`s, one by one (`ControlHelper` itself is
//! three flags and `Reset`):
//! - `_canDrawControlCircles` (`!World.Pause && WhoHasLocalControl.Count
//!   != 0`): ported: nothing while physics is paused or no car has the
//!   player's control (`NoInput`, Middleware's `RemoveControl`);
//! - the gadget picture needs a gadget, the weapon's a weapon the player
//!   fires (`!AutoFire`); the movement's shows on any car: ported;
//! - `RestrictX` / `RestrictY` hide the arrow keys' halves: ported
//!   (`SetBodyRestrictions` locking the car's translation);
//! - touch: a zone with a finger on it draws the touch itself, not the
//!   highlight; the weapon's zone also hides while the gadget's finger is
//!   down; the stick's picture hides for a static car: ported ([`Fingers`]);
//! - touch gadget picture: `GetTouchView` of the gadget: ported for the
//!   magnet (`BigCircleGadget`) and the accelerator, which has no picture
//!   until its energy is whole (`_isReady`).
//!
//! Not ported:
//! - the faint touch circles drawn all the time (0.14, 0.1), the touch
//!   itself (the ring under the finger, the stick) and the magnet's
//!   "connected" picture (`BigCircleGadgetConnected`, while it holds
//!   something): the port does not draw touch controls yet, so nothing
//!   shows when no highlight is on;
//! - the phone's dpi correction of the circles' size and place
//!   (`GetDpi`, `MaxTouchControlRadiusInInches`, the offsets of the stick
//!   and the weapon circle): the circles grow with the window's height;
//! - the gadget's special state (opacity 0.3 not highlighted, and the
//!   stick's picture hidden meanwhile);
//! - the Xbox's per-player pad and `Settings` control remapping: one device
//!   of each kind;
//! - the pictures do not turn with the camera in portrait (nor do the
//!   splashes).

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::actions::ControlHighlight;
use crate::assets::sprite_path;
use crate::controls::{ActiveScheme, FireTouch, GadgetTouch, LocalPlayer, NoInput, TouchStick};
use crate::gadget::Magnet;
use crate::gadget::accelerator::Accelerator;
use crate::game::{GameState, MapEntity};
use crate::weapon::Weapon;

/// Over the HUD's text, under the splashes.
const Z: i32 = 90;

/// The window height the layouts are made for (`Settings.ViewHeight`).
const DESIGN_HEIGHT: f32 = 480.0;

/// `controlActiveR * 1.8` at 800 × 480: the touch circles' radius.
const TOUCH_RADIUS: f32 = 56.0 * 1.8;

/// The pictures, by content name.
pub const MOUSE_LEFT: &str = "UI/MouseLeftButton";
pub const MOUSE_RIGHT: &str = "UI/MouseRightButton";
pub const KEYS_LEFT_RIGHT: &str = "UI/KeyboardControlsLeftRight";
pub const KEYS_UP_DOWN: &str = "UI/KeyboardControlsUpDown";
pub const PAD_BACK: &str = "UI/Gamepad/Gamepad_Back";
pub const PAD_STICK: &str = "UI/Gamepad/Gamepad_Stick";
pub const PAD_BUMPER: &str = "UI/Gamepad/Gamepad_RBumper";
pub const PAD_TRIGGER: &str = "UI/Gamepad/Gamepad_RTrigger";
pub const CIRCLE: &str = "UI/BigCircle";
pub const CIRCLE_GUN: &str = "UI/BigCircleGun";
pub const CIRCLE_GADGET: &str = "UI/BigCircleGadget";
pub const CIRCLE_ACCELERATOR: &str = "UI/BigCircleAccelerator";

const ALL: [&str; 12] = [
    MOUSE_LEFT,
    MOUSE_RIGHT,
    KEYS_LEFT_RIGHT,
    KEYS_UP_DOWN,
    PAD_BACK,
    PAD_STICK,
    PAD_BUMPER,
    PAD_TRIGGER,
    CIRCLE,
    CIRCLE_GUN,
    CIRCLE_GADGET,
    CIRCLE_ACCELERATOR,
];

/// What gadget the player's car has, as the touch picture needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GadgetView {
    /// The magnet (`ChainGadget`).
    Magnet,
    /// The accelerator, and whether its energy is whole.
    Accelerator { ready: bool },
}

/// The player's car, as the pictures need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Car {
    pub gadget: Option<GadgetView>,
    /// A weapon the player fires (`!AutoFire`).
    pub weapon: bool,
    pub restrict_x: bool,
    pub restrict_y: bool,
    /// `IsStatic`.
    pub is_static: bool,
}

/// Which touch zones have a finger on them ([`crate::controls`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fingers {
    pub gadget: bool,
    pub fire: bool,
    pub stick: bool,
}

/// One picture to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub sprite: &'static str,
    /// Where its middle is, logical pixels from the top left.
    pub center: Vec2,
    /// Times the picture's own size.
    pub scale: f32,
    pub red: bool,
    /// It pulses (`Triangle_Period500`), else it has `alpha`.
    pub pulse: bool,
    pub alpha: f32,
}

impl Hint {
    fn pulsing(sprite: &'static str, center: Vec2, scale: f32) -> Hint {
        Hint { sprite, center, scale, red: false, pulse: true, alpha: 1.0 }
    }
}

/// What [`hints`] needs to know.
#[derive(Debug, Clone, Copy)]
pub struct Situation {
    pub scheme: ActiveScheme,
    pub highlight: ControlHighlight,
    pub car: Car,
    pub fingers: Fingers,
    /// The window, logical pixels.
    pub screen: Vec2,
}

/// The pictures to draw now, back to front. `size` is a picture's own size
/// in pixels.
pub fn hints(s: &Situation, size: impl Fn(&str) -> Vec2) -> Vec<Hint> {
    let h = s.highlight;
    if !(h.gadget || h.weapon || h.movement) {
        return Vec::new();
    }
    let k = s.screen.y / DESIGN_HEIGHT;
    let (w, ht) = (s.screen.x, s.screen.y);
    let mut out = Vec::new();
    match s.scheme {
        ActiveScheme::Keyboard => {
            if h.gadget && s.car.gadget.is_some() {
                out.push(Hint::pulsing(MOUSE_RIGHT, Vec2::new(w - 128.0 * k, 128.0 * k), k));
            }
            if h.weapon && s.car.weapon {
                out.push(Hint::pulsing(MOUSE_LEFT, Vec2::new(w - 128.0 * k, ht - 128.0 * k), k));
            }
            if h.movement {
                let at = Vec2::new(w / 4.0, ht / 1.5);
                if !s.car.restrict_x {
                    out.push(Hint::pulsing(KEYS_LEFT_RIGHT, at, k));
                }
                if !s.car.restrict_y {
                    out.push(Hint::pulsing(KEYS_UP_DOWN, at, k));
                }
            }
        }
        ActiveScheme::Gamepad => {
            let back = size(PAD_BACK) * k;
            let base = Vec2::new(w - back.x * 0.7, back.y * 0.8);
            let top_left = base - back / 2.0;
            // Each piece's middle: its top left on the pad plus half of it.
            let at = |name: &str, offset: Vec2| top_left + offset * k + size(name) * k / 2.0;
            let mut pieces = Vec::new();
            let (mut draw_base, mut right_stick) = (false, false);
            if h.gadget && s.car.gadget.is_some() {
                pieces.push(Hint::pulsing(PAD_BUMPER, at(PAD_BUMPER, Vec2::new(180.0, 25.0)), k));
                draw_base = true;
                right_stick = true;
            }
            if h.weapon && s.car.weapon {
                pieces.push(Hint::pulsing(PAD_TRIGGER, at(PAD_TRIGGER, Vec2::new(190.0, -1.0)), k));
                draw_base = true;
                right_stick = true;
            }
            if h.movement {
                pieces.push(Hint::pulsing(PAD_STICK, at(PAD_STICK, Vec2::new(28.0, 70.0)), k));
                draw_base = true;
            }
            if right_stick {
                pieces.push(Hint::pulsing(PAD_STICK, at(PAD_STICK, Vec2::new(144.0, 106.0)), k));
            }
            if draw_base {
                out.push(Hint { sprite: PAD_BACK, center: base, scale: k, red: false, pulse: false, alpha: 0.8 });
            }
            out.extend(pieces);
        }
        ActiveScheme::Touch => {
            // `controlDrawActiveR / (BigCircle.Width / 2)`: every circle is
            // drawn at the same scale, whatever its own size.
            let r = TOUCH_RADIUS * k;
            let scale = r / (size(CIRCLE).x / 2.0);
            let (x_left, x_right) = (r * 1.25, w - r * 1.25);
            let gadget_picture = match s.car.gadget {
                Some(GadgetView::Magnet) => Some(CIRCLE_GADGET),
                Some(GadgetView::Accelerator { ready: true }) => Some(CIRCLE_ACCELERATOR),
                _ => None,
            };
            if let Some(sprite) = gadget_picture.filter(|_| h.gadget && !s.fingers.gadget) {
                out.push(Hint::pulsing(sprite, Vec2::new(x_right, ht / 2.0 - r * 1.25), scale));
            }
            if h.weapon && s.car.weapon && !s.fingers.gadget && !s.fingers.fire {
                let mut gun = Hint::pulsing(CIRCLE_GUN, Vec2::new(x_right, ht - r * 1.25), scale);
                gun.red = true;
                out.push(gun);
            }
            if h.movement && !s.car.is_static && !s.fingers.stick {
                out.push(Hint::pulsing(CIRCLE, Vec2::new(x_left, ht - r * 1.25), scale));
            }
        }
    }
    out
}

/// `PhaseHelper.Triangle_Period500`: 0 → 1 → 0 once a second.
pub fn triangle(seconds: f32) -> f32 {
    let r = (seconds % 1.0) * 2.0;
    if r > 1.0 { 2.0 - r } else { r }
}

#[derive(Resource, Default)]
struct HintSprites(Vec<(&'static str, Handle<Image>)>);

#[derive(Component)]
struct HintNode(usize);

pub struct ControlHighlightPlugin;

impl Plugin for ControlHighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HintSprites>()
            .init_resource::<ControlHighlight>()
            .add_systems(Startup, load)
            .add_systems(Update, draw.run_if(in_state(GameState::Playing)));
    }
}

fn load(mut sprites: ResMut<HintSprites>, server: Res<AssetServer>) {
    for name in ALL {
        match sprite_path(name) {
            Some(path) => sprites.0.push((name, server.load(path))),
            None => warn!("control highlight: no sprite {name}"),
        }
    }
}

type PlayerData = (
    Has<NoInput>,
    Has<Magnet>,
    Option<&'static Accelerator>,
    Option<&'static Weapon>,
    Option<&'static LockedAxes>,
    Option<&'static RigidBody>,
);

/// The pictures to draw, each with the picture's own size.
fn wanted(
    scheme: ActiveScheme,
    highlight: ControlHighlight,
    fingers: Fingers,
    window: &Window,
    player: &Query<PlayerData, With<LocalPlayer>>,
    sprites: &HintSprites,
    images: &Assets<Image>,
) -> Option<Vec<(Hint, Vec2)>> {
    // `_canDrawControlCircles` is checked by the caller; a car under control.
    let (_, magnet, accelerator, weapon, axes, body) = player.iter().find(|p| !p.0)?;
    let car = Car {
        gadget: match (magnet, accelerator) {
            (true, _) => Some(GadgetView::Magnet),
            (_, Some(a)) => Some(GadgetView::Accelerator { ready: a.ready() }),
            _ => None,
        },
        weapon: weapon.is_some_and(|w| !w.kind.auto_fire()),
        restrict_x: axes.is_some_and(|a| a.is_translation_x_locked()),
        restrict_y: axes.is_some_and(|a| a.is_translation_y_locked()),
        is_static: body.is_some_and(|b| b.is_static()),
    };
    let situation = Situation {
        scheme,
        highlight,
        car,
        fingers,
        screen: Vec2::new(window.width(), window.height()),
    };
    // Every picture must have loaded to be measured.
    let mut sizes = std::collections::HashMap::new();
    for (name, handle) in &sprites.0 {
        sizes.insert(*name, images.get(handle)?.size_f32());
    }
    let size = |name: &str| sizes.get(name).copied().unwrap_or(Vec2::ONE);
    Some(hints(&situation, size).into_iter().map(|h| { let own = size(h.sprite); (h, own) }).collect())
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    time: Res<Time>,
    (highlight, scheme, sprites, images): (Res<ControlHighlight>, Res<ActiveScheme>, Res<HintSprites>, Res<Assets<Image>>),
    (physics, stick, gadget, fire): (Res<Time<Physics>>, Res<TouchStick>, Res<GadgetTouch>, Res<FireTouch>),
    windows: Query<&Window, With<PrimaryWindow>>,
    player: Query<PlayerData, With<LocalPlayer>>,
    mut nodes: Query<(Entity, &HintNode, &mut Node, &mut ImageNode)>,
) {
    let fingers = Fingers { gadget: gadget.finger.is_some(), fire: fire.finger.is_some(), stick: stick.finger.is_some() };
    let shown = windows
        .single()
        .ok()
        // `_canDrawControlCircles`: not paused.
        .filter(|_| !physics.is_paused())
        .and_then(|window| wanted(*scheme, *highlight, fingers, window, &player, &sprites, &images))
        .unwrap_or_default();

    if nodes.iter().count() != shown.len() {
        for (e, ..) in &nodes {
            commands.entity(e).despawn();
        }
        for (i, (hint, _)) in shown.iter().enumerate() {
            let Some((_, handle)) = sprites.0.iter().find(|(n, _)| *n == hint.sprite) else {
                continue;
            };
            commands.spawn((
                Name::new(format!("Control highlight {}", hint.sprite)),
                HintNode(i),
                MapEntity,
                Node { position_type: PositionType::Absolute, ..default() },
                ImageNode::new(handle.clone()),
                GlobalZIndex(Z),
                ZIndex(i as i32),
                Pickable::IGNORE,
            ));
        }
        return;
    }
    let pulse = triangle(time.elapsed_secs());
    for (_, index, mut node, mut image) in &mut nodes {
        let Some((hint, own)) = shown.get(index.0) else {
            continue;
        };
        let size = *own * hint.scale;
        let top_left = hint.center - size / 2.0;
        node.left = Val::Px(top_left.x);
        node.top = Val::Px(top_left.y);
        node.width = Val::Px(size.x);
        node.height = Val::Px(size.y);
        let a = if hint.pulse { pulse } else { hint.alpha };
        if let Some((_, handle)) = sprites.0.iter().find(|(n, _)| *n == hint.sprite)
            && image.image != *handle
        {
            image.image = handle.clone();
        }
        image.color = if hint.red { Color::srgba(1.0, 0.0, 0.0, a) } else { Color::srgba(1.0, 1.0, 1.0, a) };
    }
}
