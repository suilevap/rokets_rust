//! The frozen glass: frost creeping over the screen on the snow maps.
//!
//! C# origin: `Map/Effects/FrozenGlassEffect.cs` (2013, `Added Frozen Glass
//! effect`, `Fix frozen effect on non standart resolutions`), made by
//! `MapEffectFactory` for `LevelEffect` `Snow` as `FrozenGlassEffect(120)`,
//! next to the falling snow (`render::snow`). The plain maps' dirty lens
//! (`render::dirty_lens`) is not made with it.
//!
//! Two sprites of frost, `FrozenGlass` (ice crystals over the whole pane)
//! and `FrozenGlass2` (a frame of frost round a clear middle), each fitted
//! to cover the canvas ([`cover`]), drawn in the middle of it over the
//! snow. The effect has an `amount` from 0 to 1, 0 at the start, that grows
//! by `1 / 120` a second (so the screen is frozen over in two minutes). The
//! crystals fade in to an alpha of 0.2 while they shrink from 1.2 times
//! their covering size, and the frame stays at an alpha of 0.5 while it
//! shrinks from twice its size (so it closes in from past the edges).
//!
//! `ChangeAmount(a)` (the `MapEffect(a)` action, see `actions::map_effect`)
//! changes how fast the frost grows, not how much there is: the speed is
//! `1 / 120` times `(a - 0.5) × 2` a second, so 1 is the usual growth, 0.5
//! stops it and 0 or less melts it, `-5` eleven times faster than it
//! grows. `LetItSnow`'s warm room (`MapEffect(-5)`) clears the glass in a
//! few seconds, and leaving it (`MapEffect(1)`) lets it grow again.
//!
//! Not ported:
//! - `World.Pause`, which held the amount (the port has no pause).
//! - `Remove` (`RemoveFromRender`): the effect's nodes go with the map.
//! - The covering scale was fixed when the class was first used
//!   (`static FrozenGlassEffect`, from `Drawer.ViewWidth`); here it follows
//!   the canvas.
//! - The linear-light blending of the veil: see
//!   [`blend_alpha`](super::effect_canvas::blend_alpha).

use bevy::prelude::*;

use super::effect_canvas::{Canvas, CanvasMade, MIDDLING_PICTURE, blend_alpha};
use super::snow::SnowSystems;
use crate::actions::MapEffectAmount;
use crate::content::map::LevelEffect;
use crate::game::{GameState, LoadedMap, MapEntity};

/// Seconds to freeze over (`new FrozenGlassEffect(120)`).
pub const SECONDS: f32 = 120.0;
/// The crystals' alpha when frozen over (`_alphaMax`).
pub const CRYSTALS_ALPHA: f32 = 0.2;
/// The frame's alpha, at the start and when frozen over (`0.5` and
/// `_alpha2Max`).
pub const FRAME_ALPHA: (f32, f32) = (0.5, 0.5);
/// The crystals' scale at the start, times the covering one (`1.2`).
pub const CRYSTALS_START: f32 = 1.2;
/// The frame's scale at the start, times the covering one (`2`).
pub const FRAME_START: f32 = 2.0;
const CRYSTALS: &str = "FrozenGlass";
const FRAME: &str = "FrozenGlass2";

/// The sprites a level's frozen glass needs, to load with the map.
pub fn textures(effect: LevelEffect) -> Vec<&'static str> {
    if super::effect_canvas::shows(effect) { vec![CRYSTALS, FRAME] } else { Vec::new() }
}

/// `Math.Max(ViewWidth / sprite.Width, ViewHeight / sprite.Height)`: the
/// scale at which a sprite of `sprite` pixels covers a canvas of `canvas`.
pub fn cover(canvas: Vec2, sprite: Vec2) -> f32 {
    (canvas.x / sprite.x).max(canvas.y / sprite.y)
}

/// `FrozenGlassEffect.ChangeAmount`: how fast the frost grows, per second,
/// for the action's `amount`.
pub fn growth_speed(amount: f32) -> f32 {
    (1.0 / SECONDS) * (amount - 0.5) * 2.0
}

/// The frost's amount after `dt` seconds at `speed` (`Update`: held between
/// 0 and 1).
pub fn grow(frost: f32, speed: f32, dt: f32) -> f32 {
    (frost + speed * dt).clamp(0.0, 1.0)
}

/// The crystals' `(scale, alpha)`, the scale times the covering one
/// (`Render`).
pub fn crystals(frost: f32) -> (f32, f32) {
    (lerp(CRYSTALS_START, 1.0, frost), lerp(0.0, CRYSTALS_ALPHA, frost))
}

/// The frame's `(scale, alpha)`, the scale times the covering one.
pub fn frame(frost: f32) -> (f32, f32) {
    (lerp(FRAME_START, 1.0, frost), lerp(FRAME_ALPHA.0, FRAME_ALPHA.1, frost))
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// What the effect is at: the frost, and how fast it grows now.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FrozenGlass {
    /// `_amount`.
    pub frost: f32,
    /// `_amountSpeed`, per second.
    pub speed: f32,
}

impl Default for FrozenGlass {
    fn default() -> Self {
        FrozenGlass { frost: 0.0, speed: growth_speed(1.0) }
    }
}

/// One of the two sprites: 0 the crystals, 1 the frame.
#[derive(Component)]
struct Pane(usize);

pub struct FrozenGlassPlugin;

impl Plugin for FrozenGlassPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), start.after(CanvasMade).after(SnowSystems::Spawned))
            .add_systems(
                Update,
                (grow_frost, show_panes).chain().run_if(in_state(GameState::Playing)).run_if(resource_exists::<FrozenGlass>),
            );
    }
}

/// Starts the frost at 0 and adds the panes over the snow in the canvas.
fn start(mut commands: Commands, canvas: Option<Res<Canvas>>, map: Res<LoadedMap>) {
    commands.remove_resource::<FrozenGlass>();
    // The canvas is also there for a plain map's speed lines.
    let Some(canvas) = canvas.filter(|_| super::effect_canvas::shows(map.data.level_effect)) else {
        return;
    };
    commands.insert_resource(FrozenGlass::default());
    commands.entity(canvas.entity).with_children(|canvas| {
        for i in 0..2 {
            canvas.spawn((
                Name::new("Frozen glass"),
                Pane(i),
                MapEntity,
                Node { position_type: PositionType::Absolute, ..default() },
                ImageNode::default(),
                UiTransform::IDENTITY,
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
        }
    });
}

fn grow_frost(time: Res<Time>, mut amounts: MessageReader<MapEffectAmount>, mut glass: ResMut<FrozenGlass>) {
    for MapEffectAmount(amount) in amounts.read() {
        glass.speed = growth_speed(*amount);
    }
    let frost = grow(glass.frost, glass.speed, time.delta_secs());
    if frost != glass.frost {
        glass.frost = frost;
    }
}

fn show_panes(
    glass: Res<FrozenGlass>,
    canvas: Res<Canvas>,
    map: Res<LoadedMap>,
    mut panes: Query<(&Pane, &mut Node, &mut ImageNode, &mut UiTransform, &mut Visibility)>,
) {
    for (pane, mut node, mut image, mut transform, mut visibility) in &mut panes {
        let Some(sprite) = map.sprites.get(if pane.0 == 0 { CRYSTALS } else { FRAME }) else {
            continue;
        };
        let (scale, alpha) = if pane.0 == 0 { crystals(glass.frost) } else { frame(glass.frost) };
        // Covering the canvas, in its middle; it shrinks about that middle.
        let size = sprite.size * cover(canvas.size, sprite.size);
        let (left, top) = ((canvas.size.x - size.x) / 2.0, (canvas.size.y - size.y) / 2.0);
        let wanted = (Val::Px(left), Val::Px(top), Val::Px(size.x), Val::Px(size.y));
        if (node.left, node.top, node.width, node.height) != wanted {
            (node.left, node.top, node.width, node.height) = wanted;
        }
        if image.image != sprite.image {
            image.image = sprite.image.clone();
        }
        image.color = Color::srgba(1.0, 1.0, 1.0, blend_alpha(alpha, MIDDLING_PICTURE));
        transform.scale = Vec2::splat(scale);
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}
