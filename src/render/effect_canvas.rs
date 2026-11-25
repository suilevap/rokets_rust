//! The screen the snow maps' effects draw on: the C# game's `DefaultUI`
//! view, a landscape picture of the window's size that the effects fill
//! (`Drawer.ViewWidth × ViewHeight`).
//!
//! C# origin: `FrozenGlassEffect` and `SnowEffect` drew in `DefaultUI`
//! pixels, scaled by `Drawer.UIDisplayUnitRatio` (`UISizeMultiplier`, how
//! many 800 × 480 views the window is the least of). Here the canvas is one
//! interface node of that size, centred on the window and turned with the
//! camera's [`ViewRotation`] (the port's portrait view): what is drawn in it
//! stays level with the map as the C# view did, and the effects need know
//! nothing of the turn.
//!
//! XNA blended sRGB numbers; Bevy blends in linear light, where a white
//! veil of a given alpha comes out much brighter than the numbers meant.
//! [`blend_alpha`] gives the alpha that comes out as XNA's over a middling
//! picture.
//!
//! Every map has the canvas too, for the speed lines ([`needed`]).
//!
//! Not ported: the effects' render targets and `Drawer.ViewWidth`
//! changing at run time (`Drawer.Init` ran once; the canvas follows the
//! window).

use bevy::prelude::*;

use crate::content::map::LevelEffect;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::camera::ViewRotation;

/// The picture the effects were drawn for (`Settings.ViewWidth × ViewHeight`).
pub const REFERENCE: Vec2 = Vec2::new(800.0, 480.0);
/// Under the HUD (80) and over the vignette (-10).
const Z: i32 = -9;
/// The sRGB number of the picture a veil is laid over when [`blend_alpha`]
/// matches it.
pub const MIDDLING_PICTURE: f32 = 0.5;

/// Whether a level has the snow's screen effects: falling snow and frozen
/// glass (`MapEffectFactory`: `Snow`; `SnowWind` has only wind-blown snow
/// in the world).
pub fn shows(effect: LevelEffect) -> bool {
    effect == LevelEffect::Snow
}

/// Whether a level has a canvas at all: every one does, for the camera's
/// speed lines (`car::speed_trail`), which C# made on every map; the snow's
/// screen effects are [`shows`].
pub fn needed(_effect: LevelEffect) -> bool {
    true
}

/// The canvas for a window of `window` logical pixels: the landscape shape
/// of it, long side across.
pub fn canvas_size(window: Vec2) -> Vec2 {
    Vec2::new(window.x.max(window.y), window.x.min(window.y))
}

/// `Drawer.UISizeMultiplier`: how much of the 800 × 480 reference the canvas
/// is, its smaller ratio.
pub fn ui_scale(canvas: Vec2) -> f32 {
    (canvas.x / REFERENCE.x).min(canvas.y / REFERENCE.y)
}

/// The alpha to blend in linear light for the result XNA's alpha `a` gave
/// on sRGB numbers, for a white veil over a picture of sRGB number
/// `picture`: the result number `p + a (1 - p)` is turned to linear light
/// and the alpha found that makes the same.
pub fn blend_alpha(a: f32, picture: f32) -> f32 {
    let a = a.clamp(0.0, 1.0);
    if a == 0.0 || picture >= 1.0 {
        return a;
    }
    let linear = |c: f32| c.powf(2.2);
    let (p, target) = (linear(picture), linear(picture + a * (1.0 - picture)));
    ((target - p) / (1.0 - p)).clamp(0.0, 1.0)
}

/// The canvas, for the effects to put their nodes in.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Canvas {
    pub entity: Entity,
    /// Its size, the window's landscape shape, logical pixels.
    pub size: Vec2,
}

impl Canvas {
    pub fn scale(&self) -> f32 {
        ui_scale(self.size)
    }
}

pub struct EffectCanvasPlugin;

/// The canvas is made; effects add their nodes after this.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanvasMade;

impl Plugin for EffectCanvasPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_canvas.in_set(CanvasMade))
            .add_systems(Update, fit_canvas.run_if(in_state(GameState::Playing)));
    }
}

fn spawn_canvas(mut commands: Commands, map: Res<LoadedMap>) {
    if !needed(map.data.level_effect) {
        commands.remove_resource::<Canvas>();
        return;
    }
    let entity = commands
        .spawn((
            Name::new("Effect canvas"),
            MapEntity,
            Node { position_type: PositionType::Absolute, ..default() },
            UiTransform::IDENTITY,
            GlobalZIndex(Z),
            Pickable::IGNORE,
        ))
        .id();
    // Sized on the first frame, before anything is drawn in it.
    commands.insert_resource(Canvas { entity, size: REFERENCE });
}

/// Keeps the canvas to the window's shape and the view's turn.
fn fit_canvas(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    view: Res<ViewRotation>,
    canvas: Option<ResMut<Canvas>>,
    mut nodes: Query<(&mut Node, &mut UiTransform)>,
) {
    let (Some(mut canvas), Some(window)) = (canvas, windows.iter().next()) else {
        return;
    };
    let window = Vec2::new(window.width(), window.height());
    if window.x <= 0.0 || window.y <= 0.0 {
        return;
    }
    let size = canvas_size(window);
    if canvas.size != size {
        canvas.size = size;
    }
    let Ok((mut node, mut transform)) = nodes.get_mut(canvas.entity) else {
        return;
    };
    let (left, top) = (Val::Px((window.x - size.x) / 2.0), Val::Px((window.y - size.y) / 2.0));
    let (width, height) = (Val::Px(size.x), Val::Px(size.y));
    if node.left != left || node.top != top || node.width != width || node.height != height {
        node.left = left;
        node.top = top;
        node.width = width;
        node.height = height;
    }
    // Interface rotation runs clockwise; the view's angle is the quarter the
    // world is turned on the screen, the same way.
    let rotation = Rot2::radians(view.angle);
    if transform.rotation != rotation {
        transform.rotation = rotation;
    }
}
