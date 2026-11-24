//! The snow maps' falling snow: `SnowEffect` over the picture (`LevelEffect`
//! `Snow`) and `WindSnowEffect` in the world (`SnowWind`).
//!
//! C# origin: `Map/Effects/SnowEffect.cs` and `WindSnowEffect.cs` (2013,
//! `Add LevelEffect snow`, `Add Wind Map Effect`, `Snow effect
//! improvments`), made by `MapEffectFactory`: `Snow` is a `CompositeEffect`
//! of `SnowEffect` and the frozen glass (`render::frozen_glass`), `SnowWind`
//! only `WindSnowEffect(4)`. `LetItSnow` and `HappyNewYear` are `Snow`; no
//! map of the campaign is `SnowWind`.
//!
//! `SnowEffect`, every update tick (60 a second), with half the effect's
//! amount as the chance: takes the way the camera is going (its look-ahead,
//! the focus less the camera's position, normalised), adds a random vector
//! of each part between -1 and 1 and halves, and asks `CreateSnowUI` for one
//! particle in the interface's pool (`MaxParticleCountUI × CountRate`, 64):
//! it starts that vector times 256 pixels from the canvas's corner, spread by
//! the canvas's size either way, and flies the way *back* (the angle plus a
//! half turn), as snow does past a moving car. The style is a streak
//! (`3Pixel`), a flake (`SnowEffect`) or a speck (`GlassDirt`) of eleven
//! (two, eight, one); [`particles::styles`](crate::particles::styles) has
//! them. Besides it draws two big slow whirls of mist (`Particles\Smoke`,
//! 40 times its size, faint, turned about a point off its middle, one at
//! each of two corners just outside the canvas, brightening and dimming in
//! turn): [`mist`].
//!
//! `WindSnowEffect(4)`, every tick: `4 × amount` times (cut to a whole
//! number) a point is picked in the view's box (`WorldViewSize / 2` about
//! the camera's position); where the floor has room (the distance field is
//! above 0) a world particle starts there, one in two a streak, else a
//! swirl, going along the nearest wall: the distance field's gradient turned
//! a quarter, shaken by an eighth of a half turn either way
//! (`CreateSnow`).
//!
//! The amounts (`ChangeMapEffectAmount`, the `MapEffect(amount)` template,
//! see `actions::map_effect`) are the snow's (`SnowEffect._amount` and
//! `WindSnowEffect._amount`, both 1 at the start): 0 or less stops the snow
//! and hides the mist.
//!
//! The screen's snow is interface nodes in the canvas
//! ([`effect_canvas`](super::effect_canvas)), one node for each of the pool's
//! slots, moved by the pool's particles each frame; the wind's is in the
//! world pool (`render::particles`).
//!
//! No map or action changes how the car drives on snow (`LevelEffect` is
//! read only by the effects and by `EntitiesManager.CreateCar`, which gives
//! the car's `SpeedTrail` component to plain maps only).
//!
//! Not ported:
//! - The update tick: C# spawned in 1/60 s steps with a chance per step;
//!   here a frame makes as many as the steps it spans would
//!   ([`screen_spawns`], [`wind_spawns`]), so the rate is the same at any
//!   frame rate.
//! - `RandomHelper`'s *fake* random numbers (a table of numbers drawn in
//!   turn): `ViewRng` is a plain generator, the same distribution.
//! - `World.Pause` (the C# game stopped the snow while paused; the port has
//!   no pause) and `World.TimeScale` (the port has none).
//! - `SnowEffect._wind`, `_updateWind` and the commented wind drift: set or
//!   commented out, never read.
//! - `CreateSnow`'s `Camera.IsInView(pos, 0)`: the point is picked inside
//!   the view's box, so it is always in view. The box is estimated from the
//!   camera's height over the ground (the port's camera is a tilted
//!   perspective one), a little smaller than what the camera sees.
//! - The look-ahead is the eased focus less the camera's ground point
//!   (`Camera.Focus.Position - Camera.Position`); under a thousandth of a
//!   unit it counts as none, where C# normalised whatever was left.
//! - The speed of a screen particle is in pixels as C#'s was, not scaled by
//!   the window's size; its size is (`UISizeMultiplier`).
//! - The linear-light blending of the veil: see [`blend_alpha`].

use std::f32::consts::{FRAC_PI_4, FRAC_PI_8, PI, TAU};

use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::camera::{CAMERA_HEIGHT, MainCamera};
use super::effect_canvas::{Canvas, CanvasMade, MIDDLING_PICTURE, blend_alpha};
use super::particles::Particles;
use crate::actions::MapEffectAmount;
use crate::ai::SharedTerrain;
use crate::content::map::LevelEffect;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::{SCREEN_SNOW, WIND_SNOW, WIND_SNOW_TURNING};
use crate::particles::{Pool, Style};

/// `MaxParticleCountUI × CountRate`: the interface's pool.
pub const SCREEN_POOL: usize = 64;
/// How many update ticks a second C# made (`Settings.TargetFps`).
pub const TICKS_PER_SECOND: f32 = 60.0;
/// The chance of a flake at each tick, at amount 1 (`RandomEvent(_amount *
/// 0.5f)`).
pub const FLAKE_CHANCE: f32 = 0.5;
/// How far the spawn spread is shifted along the way the camera goes, pixels
/// (`CreateSnowUI`'s `l`).
pub const LEAD: f32 = 256.0;
/// `WindSnowEffect(4)`: points picked at each tick, at amount 1.
pub const WIND_POINTS: usize = 4;
/// `WindSnowEffect`'s swirl: a half of the starts (`RandomEvent(0.5f)`).
const SWIRL_CHANCE: f32 = 0.5;
/// How much the look-ahead has to be to count (see the module's not
/// ported).
const MIN_LOOK: f32 = 1e-3;

/// The mist's size, in times the sprite's (`40 * UIDisplayUnitRatio`, drawn
/// as `40 * UISizeMultiplier`).
pub const MIST_SCALE: f32 = 40.0;
/// How far the sprite's turning point is from its middle, in the sprite's
/// pixels (`_smokeView.Origin += Vector2.UnitX * 8`).
pub const MIST_PIVOT: f32 = 8.0;
/// Where the mist's corners are outside the canvas, pixels.
pub const MIST_OUT: f32 = 64.0;
/// How faint the mist is at most.
pub const MIST_ALPHA: f32 = 0.15;
const MIST: &str = "Particles\\Smoke";

/// The sprites a level's snow needs, to load with the map.
pub fn textures(effect: LevelEffect) -> Vec<&'static str> {
    match effect {
        LevelEffect::None => Vec::new(),
        LevelEffect::Snow => {
            let mut names: Vec<&str> = SCREEN_SNOW.iter().map(|s| s.texture).collect();
            names.push(MIST);
            names.dedup();
            names
        }
        LevelEffect::SnowWind => vec![WIND_SNOW.texture, WIND_SNOW_TURNING.texture],
    }
}

/// How many screen snow particles a frame of `dt` seconds starts at
/// `amount`: each of the ticks it spans has the chance [`FLAKE_CHANCE`] ×
/// `amount` (none at 0 or less), `roll` (0 to 1, random) settling the
/// fraction.
pub fn screen_spawns(amount: f32, dt: f32, roll: f32) -> usize {
    if amount <= 0.0 {
        return 0;
    }
    ((amount * FLAKE_CHANCE * dt * TICKS_PER_SECOND) + roll).floor().max(0.0) as usize
}

/// How many points a frame of `dt` seconds tries for the wind snow:
/// `(int)(max × amount)` at each tick it spans.
pub fn wind_spawns(max: usize, amount: f32, dt: f32, roll: f32) -> usize {
    let per_tick = (max as f32 * amount).max(0.0).trunc();
    ((per_tick * dt * TICKS_PER_SECOND) + roll).floor().max(0.0) as usize
}

/// The way the camera goes, as a direction (`spv2.Normalize()`), or none.
pub fn lead_direction(look: Vec2) -> Vec2 {
    if look.length() < MIN_LOOK { Vec2::ZERO } else { look.normalize() }
}

/// Where a screen snow particle starts and which way it goes (`CreateSnowUI`),
/// on the canvas with y up from its top-left corner (the pool's own axes;
/// the canvas's point is `(x, -y)`): `lead` is the camera's
/// [`lead_direction`], `jitter` and `spread` two random vectors of parts
/// between -1 and 1.
pub fn screen_start(lead: Vec2, jitter: Vec2, spread: Vec2, canvas: Vec2) -> (Vec2, f32) {
    let v = (lead + jitter) / 2.0;
    (v * LEAD + spread * canvas, v.to_angle() + PI)
}

/// The point of the canvas, y down, of a point of the pool.
pub fn canvas_point(pool_point: Vec2) -> Vec2 {
    Vec2::new(pool_point.x, -pool_point.y)
}

/// `PhaseHelper.GetTrianglePhase(time, 1000)`: 0 to 1 and back, once a
/// second.
pub fn triangle(seconds: f32) -> f32 {
    let t = seconds.rem_euclid(1.0) * 2.0;
    if t > 1.0 { 2.0 - t } else { t }
}

/// One of the two whirls of mist, in the canvas's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MistPlace {
    /// Where the sprite's middle is.
    pub center: Vec2,
    /// How far it is turned clockwise.
    pub rotation: f32,
    /// How opaque it is.
    pub alpha: f32,
}

/// `SnowEffect.Render`'s whirl `which` (0 or 1) `seconds` into the map, on a
/// canvas of `canvas` pixels at `scale` (`UISizeMultiplier`), for the
/// effect's `amount`. The first sits above and left of the canvas and turns
/// back a radian a second, the second below and right, a radian in 1.2 s
/// the other way; the first is brightest when the second is faintest.
pub fn mist(which: usize, seconds: f32, canvas: Vec2, scale: f32, amount: f32) -> MistPlace {
    let phase = triangle(seconds);
    let (corner, rotation, alpha) = if which == 0 {
        (Vec2::splat(-MIST_OUT), -seconds % TAU, 0.5 + phase * 0.5)
    } else {
        (canvas + Vec2::splat(MIST_OUT), (seconds / 1.2) % TAU, 1.0 - phase * 0.5)
    };
    // The sprite turns about a point a few of its pixels from its middle.
    let pivot = MIST_PIVOT * MIST_SCALE * scale;
    MistPlace {
        center: corner + Vec2::new(-pivot * rotation.cos(), -pivot * rotation.sin()),
        rotation,
        alpha: MIST_ALPHA * alpha * amount,
    }
}

/// The box the wind snow starts in, half of its width and height, in world
/// units: the camera `height` over the ground, on a canvas whose long side
/// is `ratio` times its short one (the view in C#'s landscape axes, the
/// short side as much of the map as the camera's field of view shows).
pub fn view_half(height: f32, ratio: f32) -> Vec2 {
    let short = height * (FRAC_PI_4 / 2.0).tan();
    Vec2::new(short * ratio, short)
}

/// Which way a wind snow particle starts (`WindSnowEffect`): along the
/// nearest wall, the quarter turn from the way the room grows (`gradient`,
/// the distance field's), shaken by `shake` (-1 to 1) of an eighth of a half
/// turn.
pub fn wind_direction(gradient: Vec2, shake: f32) -> f32 {
    gradient.to_angle() + PI / 2.0 + shake * FRAC_PI_8
}

/// What the snow's randomness draws from: the generator for what is only
/// seen.
#[derive(Resource)]
struct ViewRng(SmallRng);

/// The screen snow of a `Snow` map: its particles and strength.
#[derive(Resource)]
struct ScreenSnow {
    pool: Pool,
    /// `SnowEffect._amount`.
    amount: f32,
}

/// The wind snow of a `SnowWind` map.
#[derive(Resource)]
struct WindSnow {
    /// `WindSnowEffect._amount`.
    amount: f32,
}

/// One of the pool's slots, drawn.
#[derive(Component)]
struct Flake;

/// A whirl of mist: 0 or 1.
#[derive(Component)]
struct Whirl(usize);

/// The snow's nodes are in the canvas; what is drawn over them goes in after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SnowSystems {
    Spawned,
}

pub struct SnowPlugin;

impl Plugin for SnowPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ViewRng(SmallRng::seed_from_u64(31)))
            .add_systems(OnEnter(GameState::Playing), (start_screen_snow.after(CanvasMade).in_set(SnowSystems::Spawned), start_wind_snow))
            .add_systems(
                Update,
                (
                    (spawn_flakes, show_flakes, show_mist).chain().run_if(resource_exists::<ScreenSnow>),
                    wind_snow.run_if(resource_exists::<WindSnow>),
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn start_screen_snow(mut commands: Commands, canvas: Option<Res<Canvas>>) {
    commands.remove_resource::<ScreenSnow>();
    let Some(canvas) = canvas else {
        return;
    };
    commands.insert_resource(ScreenSnow { pool: Pool::new(SCREEN_POOL), amount: 1.0 });
    let slot = |_: usize| {
        (
            Name::new("Snow"),
            Flake,
            MapEntity,
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), ..default() },
            ImageNode::default(),
            UiTransform::IDENTITY,
            Visibility::Hidden,
            Pickable::IGNORE,
        )
    };
    let whirl = |i: usize| {
        (
            Name::new("Mist"),
            Whirl(i),
            MapEntity,
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), ..default() },
            ImageNode::default(),
            UiTransform::IDENTITY,
            Visibility::Hidden,
            Pickable::IGNORE,
        )
    };
    commands.entity(canvas.entity).with_children(|canvas| {
        for i in 0..SCREEN_POOL {
            canvas.spawn(slot(i));
        }
        for i in 0..2 {
            canvas.spawn(whirl(i));
        }
    });
}

fn start_wind_snow(mut commands: Commands, map: Res<LoadedMap>) {
    if map.data.level_effect == LevelEffect::SnowWind {
        commands.insert_resource(WindSnow { amount: 1.0 });
    } else {
        commands.remove_resource::<WindSnow>();
    }
}

fn spawn_flakes(
    time: Res<Time>,
    mut amounts: MessageReader<MapEffectAmount>,
    mut snow: ResMut<ScreenSnow>,
    mut rng: ResMut<ViewRng>,
    canvas: Option<Res<Canvas>>,
    cameras: Query<&MainCamera>,
) {
    for MapEffectAmount(amount) in amounts.read() {
        snow.amount = *amount;
    }
    let dt = time.delta_secs();
    snow.pool.step(dt);
    let (Some(canvas), Ok(camera)) = (canvas, cameras.single()) else {
        return;
    };
    let lead = lead_direction(camera.focus - camera.ground);
    let rng = &mut rng.0;
    let count = screen_spawns(snow.amount, dt, rng.random()).min(SCREEN_POOL);
    for _ in 0..count {
        let mut unit = || Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0);
        let (jitter, spread) = (unit(), unit());
        let style: &'static Style = SCREEN_SNOW[(rng.random::<f32>() * SCREEN_SNOW.len() as f32) as usize % SCREEN_SNOW.len()];
        let (at, direction) = screen_start(lead, jitter, spread, canvas.size);
        snow.pool.try_add(style, at, direction, direction);
    }
}

type FlakeNode = (&'static mut Node, &'static mut ImageNode, &'static mut UiTransform, &'static mut Visibility);

fn show_flakes(
    snow: Res<ScreenSnow>,
    canvas: Res<Canvas>,
    map: Res<LoadedMap>,
    mut nodes: Query<FlakeNode, With<Flake>>,
) {
    let k = canvas.scale();
    let mut particles = snow.pool.iter();
    for (mut node, mut image, mut transform, mut visibility) in &mut nodes {
        let drawn = particles.next().and_then(|p| Some((p, map.sprites.get(p.style.texture)?)));
        let Some((p, sprite)) = drawn else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        // A row of square frames: one of them (`UseRandomFrame`).
        let frames = if p.style.random_frame { (sprite.size.x / sprite.size.y).round().max(1.0) } else { 1.0 };
        let frame = Vec2::new(sprite.size.x / frames, sprite.size.y);
        let size = frame * k;
        let (width, height) = (Val::Px(size.x), Val::Px(size.y));
        if node.width != width || node.height != height {
            node.width = width;
            node.height = height;
        }
        if image.image != sprite.image {
            image.image = sprite.image.clone();
        }
        let f = (p.frame % frames as u32) as f32;
        image.rect = p.style.random_frame.then(|| Rect::new(f * frame.x, 0.0, (f + 1.0) * frame.x, frame.y));
        image.color = Color::srgba(1.0, 1.0, 1.0, blend_alpha(p.state.color[3], MIDDLING_PICTURE));
        // The node sits in the canvas's corner; its middle goes to the
        // particle (rotation and scale are about the middle).
        let centre = canvas_point(p.position);
        transform.translation = Val2::px(centre.x - size.x / 2.0, centre.y - size.y / 2.0);
        transform.scale = p.state.scale.max(Vec2::splat(1e-3));
        // The pool turns counter-clockwise on a y-up canvas; interface
        // rotation is clockwise.
        transform.rotation = Rot2::radians(-p.view_direction);
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}

fn show_mist(
    time: Res<Time>,
    snow: Res<ScreenSnow>,
    canvas: Res<Canvas>,
    map: Res<LoadedMap>,
    mut nodes: Query<(&Whirl, &mut Node, &mut ImageNode, &mut UiTransform, &mut Visibility)>,
) {
    let Some(sprite) = map.sprites.get(MIST) else {
        return;
    };
    let k = canvas.scale();
    for (whirl, mut node, mut image, mut transform, mut visibility) in &mut nodes {
        // `if (_amount <= 0) return;`: nothing drawn.
        let shown = if snow.amount > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != shown {
            *visibility = shown;
        }
        if snow.amount <= 0.0 {
            continue;
        }
        let place = mist(whirl.0, time.elapsed_secs(), canvas.size, k, snow.amount);
        let size = sprite.size * k;
        let (width, height) = (Val::Px(size.x), Val::Px(size.y));
        if node.width != width || node.height != height {
            node.width = width;
            node.height = height;
        }
        if image.image != sprite.image {
            image.image = sprite.image.clone();
        }
        image.color = Color::srgba(1.0, 1.0, 1.0, blend_alpha(place.alpha, MIDDLING_PICTURE));
        transform.translation = Val2::px(place.center.x - size.x / 2.0, place.center.y - size.y / 2.0);
        transform.scale = Vec2::splat(MIST_SCALE);
        transform.rotation = Rot2::radians(place.rotation);
    }
}

#[allow(clippy::too_many_arguments)]
fn wind_snow(
    time: Res<Time>,
    mut amounts: MessageReader<MapEffectAmount>,
    mut wind: ResMut<WindSnow>,
    mut rng: ResMut<ViewRng>,
    terrain: Option<Res<SharedTerrain>>,
    cameras: Query<&MainCamera>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut particles: ResMut<Particles>,
) {
    for MapEffectAmount(amount) in amounts.read() {
        wind.amount = *amount;
    }
    let (Some(terrain), Ok(camera), Some(window)) = (terrain, cameras.single(), windows.iter().next()) else {
        return;
    };
    let (w, h) = (window.width(), window.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let half = view_half(CAMERA_HEIGHT / camera.scale, w.max(h) / w.min(h));
    let rng = &mut rng.0;
    let tries = wind_spawns(WIND_POINTS, wind.amount, time.delta_secs(), rng.random()).min(128);
    for _ in 0..tries {
        let at = camera.focus + half * Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0);
        if terrain.0.distance_at(at) <= 0.0 {
            continue;
        }
        let direction = wind_direction(terrain.0.gradient_at(at), rng.random::<f32>() * 2.0 - 1.0);
        let style = if rng.random::<f32>() < SWIRL_CHANCE { &WIND_SNOW } else { &WIND_SNOW_TURNING };
        particles.try_add(style, at, direction, direction);
    }
}
