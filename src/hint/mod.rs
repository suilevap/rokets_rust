//! Hints: the arrows, warnings, texts and countdowns a map puts over the
//! world (`<Hints>`, `HintData`; `Entities/HintTile.cs`,
//! `EntitiesManager.CreateHintTile`, `MapManager`'s creation of them).
//!
//! A hint is off until it is switched on, by the player's car standing in
//! its `Area` (checked once a second) or by `ActivateEntity(id)`; it then
//! flickers in over a second, blinks while it stays (a text does not), and
//! fades out over a second when switched off (`DeactivateEntity(id)`, or
//! the player leaving the area). `DestroyEntity(id)` removes it with a
//! burst of white specks. What it shows:
//!
//! - **a countdown** (`Timer` above 0): the time left, one decimal, from
//!   the moment it is switched on, stopping at 0;
//! - **a text** (`Text`, a key in the game's texts): the text, left at its
//!   place and turned by `DirectionDeg`;
//! - **else its sprite** (`Sprite`), a row of square frames when
//!   `Animated`, pulsing in size by `PulseAmount`.
//!
//! In the world (the default) it is drawn over everything, unlit. With
//! `DrawOnUI` it is drawn on the screen at the place the hint's position
//! has there, at a fixed size (`Drawer.UIDisplayUnitRatio`), whatever the
//! zoom.
//!
//! `state` is the whole behaviour as plain data (tested); this module
//! reads the player and the clock, gives the hints their entities and draws
//! them: sprites as quads in the world (`render/sprite`), texts and
//! on-screen sprites as Bevy UI nodes in `PostUpdate`, placed over the
//! world's point through the camera.
//!
//! Not ported:
//! - the C# hint is drawn only while its centre is within its radius of
//!   the view (`Camera.IsInView`); here the world sprite is culled by
//!   Bevy, a text or on-screen sprite is placed wherever it projects and
//!   clipped by the window, and the hint's animation and countdown run
//!   whether it is in view or not (C#'s animation only advanced while
//!   drawn: one that was out of view picked up where it left, here it has
//!   moved on; its countdown ran either way);
//! - hints are drawn in the foreground pass with no depth test, over the
//!   walls too; a world sprite here is hidden by a wall in front of it as
//!   every sprite is, and a text always shows;
//! - the area is checked against the position of the local player's car;
//!   once it is wrecked (it no longer has [`LocalPlayer`]) no check is
//!   made and a hint stays as it was, where C# kept checking
//!   `MainSubscriber` while it was in the world;
//! - the periodic area check, the blink and the pulse count game seconds
//!   (they slow down and pause with the game), the C# check and the
//!   countdown counted real time (`TotalGameTime` and an unscaled
//!   `ElapsedGameTime`), so slow motion slowed the pulse but not them;
//! - a text whose key the game's texts lack shows `#key`, where C# showed
//!   `!NL:key` (the port's texts do the same for every text); the text is
//!   looked up every frame, C# looked it up once when the map began;
//! - a text is sized as the splashes' texts are (the Title font at 40
//!   pixels, see `splash`), not by the font's own measure (`Fonts.Scale`);
//!   the world text is scaled and turned by the camera's view of its place, so it is
//!   drawn flat as a UI node, not tilted with the perspective;
//! - an on-screen hint is not turned with a portrait view (C# had no such
//!   view), and its place is the camera's projection of the position;
//! - the destroy splash is made wherever the hint is, in view or not
//!   (`Camera.IsInView` guarded it), and its specks are random rather than
//!   from the fake-random table;
//! - `RandomEvent` draws from a real random generator, not the fake table
//!   (the flicker is random either way);
//! - a hint switched off before it has been updated once (`Activate` and
//!   `Deactivate` in the same frame) goes off at its next update; in C# the
//!   fade-out only ran while the fade time was above 0, so such a hint
//!   stayed on for good, a flaw not worth keeping;
//! - the text's `Radius` (it only fed the in-view test) is not kept, and a
//!   hint has no physics body (C#'s had one, disabled): nothing else
//!   can bump it, and `Deactivate` leaves no body to disable;
//! - `ActionOnCreate` / `ActionOnDestroy` of a hint are not read (C#
//!   returned nothing for a `HintTile`; the map data has none for them).

pub mod state;

use avian2d::prelude::Position;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy::window::PrimaryWindow;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::actions::area::in_area;
use crate::actions::{Activation, OwnActivation};
use crate::content::map::HintData;
use crate::content::xna::XRect;
use crate::controls::LocalPlayer;
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::info::{GameFont, Texts};
use crate::particles::{State, Style};
use crate::render::camera::{CameraFollow, MainCamera};
use crate::render::effect_canvas::{canvas_size, ui_scale};
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteInfo, SpriteTint, UnitQuad, sprite_bundle};
use crate::splash::TITLE_PX;
use crate::units::{PX_PER_UNIT, px};
use state::{Frames, HintState, triangle};

/// Over the player HUD (80).
const Z: i32 = 90;

/// What a hint shows (`HintTile.Draw`'s branches, in the order it tries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// The countdown (`Timer` above 0).
    Countdown,
    /// The text (`Text` is a key).
    Text,
    /// The sprite.
    Sprite,
}

/// A hint.
#[derive(Component, Debug, Clone)]
pub struct Hint {
    pub state: HintState,
    pub look: Look,
    /// The text's key.
    pub text: Option<String>,
    /// Where the player switches it on, in map cells; no width or height
    /// means never.
    pub area: XRect,
    pub draw_on_ui: bool,
    /// `Scale`, the size of its sprite and text.
    pub scale: f32,
    /// A sprite's frames, when it is one.
    pub frames: Option<Frames>,
}

impl Hint {
    /// A hint of the map's data. `frame_count` is how many square frames
    /// its sprite has (1 for a plain picture).
    pub fn new(data: &HintData, frame_count: usize) -> Hint {
        let text = data.text_key().map(str::to_string);
        let look = if data.timer > 0.0 {
            Look::Countdown
        } else if text.is_some() {
            Look::Text
        } else {
            Look::Sprite
        };
        Hint {
            state: HintState::new(data.timer, text.is_some(), data.pulse_amount),
            look,
            text,
            area: data.area,
            draw_on_ui: data.draw_on_ui,
            scale: data.scale,
            frames: (look == Look::Sprite && data.animated)
                .then(|| Frames::new(frame_count, data.animation_sequence.as_deref())),
        }
    }

    /// Whether the player's check applies: an area with no width or height
    /// has none (`new PeriodicUpdate` only for a real one).
    pub fn has_area(&self) -> bool {
        self.area.w > 0 && self.area.h > 0
    }

    /// Whether it is drawn now (`IsActive` and `Visible`).
    pub fn shown(&self) -> bool {
        self.state.active && self.state.visible
    }
}

/// A piece of a hint that draws it; it stays while the hint does, and
/// the hint's end is found by its hint being gone.
#[derive(Component, Debug, Clone, Copy)]
struct View {
    hint: Entity,
    /// Where the hint stood, world, its direction (radians, counter-
    /// clockwise) and its size in world units: for the splash it leaves.
    at: Vec2,
    direction: f32,
    size: Vec2,
}

/// The hint's sprite as a quad in the world.
#[derive(Component, Debug, Clone)]
struct WorldSprite {
    /// Its size at `Scale` 1, world units.
    base: Vec2,
    /// The row's frame count, and the frame the picture shows now.
    frames: usize,
    shown_frame: Option<usize>,
}

/// The hint's sprite on the screen.
#[derive(Component, Debug, Clone)]
struct ScreenSprite {
    /// One frame's size in the picture's pixels, and the row's count.
    frame: Vec2,
    frames: usize,
}

/// A text or a countdown as a UI node.
#[derive(Component, Debug, Clone, Default)]
struct Label {
    /// The width of the first text it showed: a countdown is drawn from
    /// the middle of that, so its digits do not shift it.
    first_width: Option<f32>,
}

/// `ParticleSystemManager._hintDestroySplash`: a white speck of a cell's
/// size shrinking away in three quarters of a second.
static DESTROY_SPLASH: Style = Style {
    begin: State {
        speed: 1.0,
        scale: Vec2::splat(32.0),
        view_rotation: 0.0,
        rotation: 0.0,
        color: [1.0; 4],
        height: -96.0 / 32.0,
    },
    end: State {
        speed: 1.0,
        scale: Vec2::ZERO,
        view_rotation: 0.0,
        rotation: 0.0,
        color: [1.0; 4],
        height: -96.0 / 32.0,
    },
    lifetime: 0.75,
    texture: "WhitePixel",
    additive: false,
    random_frame: false,
    origin: Vec2::splat(0.5),
};

/// The picture every hint of the map uses.
pub const SPLASH_SPRITE: &str = "WhitePixel";

/// The sprites a map's hints draw, to load with it: each hint's own,
/// and the speck of the burst they leave when destroyed.
pub fn sprites_of(hints: &[HintData]) -> Vec<String> {
    if hints.is_empty() {
        return Vec::new();
    }
    hints
        .iter()
        .filter(|h| h.timer <= 0.0 && h.text_key().is_none())
        .map(|h| h.sprite.clone())
        .chain([SPLASH_SPRITE.to_string()])
        .collect()
}

pub struct HintPlugin;

impl Plugin for HintPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_hints)
            .add_systems(
                Update,
                (check_area, apply_activations, step_hints, end_views)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                PostUpdate,
                (show_world_sprites.before(TransformSystems::Propagate), (place_labels, place_screen_sprites).after(CameraFollow).before(UiSystems::Layout))
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// `MapManager`'s `CreateHintTile` for each hint, once the entities of the
/// map are made.
#[allow(clippy::too_many_arguments)]
fn spawn_hints(
    mut commands: Commands,
    map: Res<LoadedMap>,
    font: Res<GameFont>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for data in map.data.hints.iter() {
        let sprite = map.sprites.get(&data.sprite);
        // `Animation`: square frames in a row, as many as fit.
        let (frame, frame_count) = match sprite {
            Some(s) if data.animated => (Vec2::splat(s.size.y), ((s.size.x / s.size.y) as usize).max(1)),
            Some(s) => (s.size, 1),
            None => (Vec2::ZERO, 1),
        };
        let hint = Hint::new(data, frame_count);
        let at = map.world(data.position.x, data.position.y);
        let direction = crate::units::map_degrees_to_world(data.direction_deg);
        let size = frame * px(1.0) * data.scale;
        let look = hint.look;
        let on_ui = hint.draw_on_ui;
        let entity = commands
            .spawn((
                Name::new(format!("Hint {}", data.id)),
                MapEntity,
                MapId(data.id),
                OwnActivation,
                hint,
                Transform::from_translation(at.extend(0.0)).with_rotation(Quat::from_rotation_z(direction)),
            ))
            .id();
        let view = View { hint: entity, at, direction, size };
        match look {
            Look::Countdown | Look::Text => {
                let first = match (look, data.text_key()) {
                    (Look::Text, Some(key)) => key.to_string(),
                    _ => state::Countdown { set: data.timer, remains: data.timer }.text(),
                };
                commands.spawn((
                    Name::new(format!("Hint {} label", data.id)),
                    MapEntity,
                    view,
                    Label::default(),
                    Text::new(first),
                    TextFont { font: font.0.clone().into(), font_size: FontSize::Px(TITLE_PX), ..default() },
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Left),
                    Node { position_type: PositionType::Absolute, ..default() },
                    UiTransform::IDENTITY,
                    GlobalZIndex(Z),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ));
            }
            Look::Sprite => {
                let Some(info) = sprite else {
                    warn!("hint {}: no sprite {:?}", data.id, data.sprite);
                    continue;
                };
                if on_ui {
                    commands.spawn((
                        Name::new(format!("Hint {} sprite", data.id)),
                        MapEntity,
                        view,
                        ScreenSprite { frame, frames: frame_count },
                        ImageNode::new(info.image.clone()),
                        Node { position_type: PositionType::Absolute, ..default() },
                        UiTransform::IDENTITY,
                        GlobalZIndex(Z),
                        Visibility::Hidden,
                        Pickable::IGNORE,
                    ));
                } else {
                    let one_frame = SpriteInfo { image: info.image.clone(), size: frame };
                    let tint = SpriteTint { color: [1.0; 4], lit: false };
                    let (mesh, material, mut transform, tint) =
                        sprite_bundle(&one_frame, data.scale, Layer::Hint, tint, &quad, &mut materials);
                    transform.translation = at.extend(Layer::Hint.z());
                    transform.rotation = Quat::from_rotation_z(direction);
                    commands.spawn((
                        Name::new(format!("Hint {} sprite", data.id)),
                        MapEntity,
                        view,
                        WorldSprite { base: transform.scale.truncate(), frames: frame_count, shown_frame: None },
                        mesh,
                        material,
                        transform,
                        tint,
                        Visibility::Hidden,
                    ));
                }
            }
        }
    }
}

/// `HintTile.Update`'s first step: every second, the local player's car in
/// the area switches the hint on and out of it off.
pub fn check_area(
    time: Res<Time>,
    map: Res<LoadedMap>,
    player: Query<&Position, With<LocalPlayer>>,
    mut hints: Query<&mut Hint>,
) {
    let now = time.elapsed_secs();
    let at = player.iter().next().map(|p| p.0);
    for mut hint in &mut hints {
        if !hint.has_area() {
            continue;
        }
        let area = hint.area;
        if hint.state.check_due(now)
            && let Some(at) = at
        {
            let inside = in_area(area, map.cell, at);
            hint.state.player_is_in_area(inside);
        }
    }
}

/// `ActivateEntity(id)` and `DeactivateEntity(id)` for a hint:
/// `Activate` and `Deactivate` of it.
pub fn apply_activations(mut messages: MessageReader<Activation>, mut hints: Query<&mut Hint>) {
    for a in messages.read() {
        if let Ok(mut hint) = hints.get_mut(a.entity) {
            if a.deactivate {
                hint.state.deactivate();
            } else {
                hint.state.activate();
            }
        }
    }
}

/// `HintTile.Update` for the rest, and the frames its sprite shows while
/// it is drawn.
pub fn step_hints(time: Res<Time>, mut rng: Local<Option<SmallRng>>, mut hints: Query<&mut Hint>) {
    let rng = rng.get_or_insert_with(|| SmallRng::seed_from_u64(31));
    let dt = time.delta_secs();
    let phase = triangle(time.elapsed_secs());
    for mut hint in &mut hints {
        hint.state.update(dt, phase, |p| rng.random::<f32>() < p);
        if hint.shown()
            && let Some(frames) = &mut hint.frames
        {
            frames.advance(dt);
        }
    }
}

/// `HintTile.Destroy`: the hint is gone (a `DestroyEntity`, never leaving
/// the map, which takes the views with it): scatter the speck burst over
/// where it stood and take its views away.
fn end_views(
    mut commands: Commands,
    hints: Query<(), With<Hint>>,
    views: Query<(Entity, &View)>,
    mut particles: ResMut<Particles>,
    mut rng: Local<Option<SmallRng>>,
    mut done: Local<Vec<Entity>>,
) {
    let rng = rng.get_or_insert_with(|| SmallRng::seed_from_u64(37));
    done.clear();
    for (e, view) in &views {
        if hints.contains(view.hint) {
            continue;
        }
        commands.entity(e).despawn();
        if done.contains(&view.hint) {
            continue;
        }
        done.push(view.hint);
        for _ in 0..10 {
            let local = Vec2::new(rng.random::<f32>() - 0.5, rng.random::<f32>() - 0.5) * view.size;
            let at = view.at + Vec2::from_angle(view.direction).rotate(local);
            let (dir, angle) = (rng.random::<f32>() * std::f32::consts::TAU, rng.random::<f32>() * std::f32::consts::TAU);
            particles.try_add(&DESTROY_SPLASH, at, dir, angle);
        }
    }
}

/// A hint's quad in the world: visible while shown, its opacity and size
/// the hint's, the frame the animation is at.
#[allow(clippy::type_complexity)]
fn show_world_sprites(
    hints: Query<&Hint>,
    mut sprites: Query<(&View, &mut WorldSprite, &mut Transform, &mut SpriteTint, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (view, mut sprite, mut transform, mut tint, mut visibility, material) in &mut sprites {
        let Ok(hint) = hints.get(view.hint) else {
            continue;
        };
        visibility.set_if_neq(if hint.shown() { Visibility::Inherited } else { Visibility::Hidden });
        if !hint.shown() {
            continue;
        }
        tint.color[3] = hint.state.opacity;
        transform.scale = (sprite.base * hint.state.size).extend(1.0);
        let frame = hint.frames.as_ref().map_or(0, Frames::current).min(sprite.frames - 1);
        if sprite.shown_frame != Some(frame)
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            let n = sprite.frames as f32;
            m.uv_transform = bevy::math::Affine2::from_scale_angle_translation(Vec2::new(1.0 / n, 1.0), 0.0, Vec2::new(frame as f32 / n, 0.0));
            sprite.shown_frame = Some(frame);
        }
    }
}

/// The camera, as the UI nodes need it to find where a world point is on
/// the screen.
struct Projector<'a> {
    camera: &'a Camera,
    transform: GlobalTransform,
}

impl Projector<'_> {
    fn point(&self, world: Vec2) -> Option<Vec2> {
        self.camera.world_to_viewport(&self.transform, world.extend(Layer::Hint.z())).ok()
    }

    /// The screen position of a world point, how many logical pixels one
    /// world unit is there, and the angle (clockwise on screen, y down) the
    /// world direction `direction` points at.
    fn at(&self, world: Vec2, direction: f32) -> Option<(Vec2, f32, f32)> {
        let here = self.point(world)?;
        let along = self.point(world + Vec2::from_angle(direction))? - here;
        let across = self.point(world + Vec2::X)? - here;
        Some((here, across.length(), along.y.atan2(along.x)))
    }
}

/// A text or countdown's node: the words, the colour and, over the
/// world's point, the size, turn and place.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn place_labels(
    hints: Query<&Hint>,
    texts: Res<Texts>,
    cameras: Query<(&Camera, &Transform), With<MainCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut labels: Query<(&View, &mut Label, &mut Text, &mut TextColor, &mut Node, &mut UiTransform, &mut Visibility, &ComputedNode)>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let projector = Projector { camera, transform: GlobalTransform::from(*camera_transform) };
    let ui = windows.iter().next().map_or(1.0, |w| ui_scale(canvas_size(Vec2::new(w.width(), w.height()))));
    for (view, mut label, mut text, mut color, mut node, mut transform, mut visibility, computed) in &mut labels {
        let Ok(hint) = hints.get(view.hint) else {
            continue;
        };
        let words = match (&hint.state.countdown, hint.look) {
            (Some(c), Look::Countdown) => c.text(),
            _ => hint.text.as_deref().map(|k| texts.0.get(k)).unwrap_or_default(),
        };
        if text.0 != words {
            text.0 = words;
        }
        // Laid out in physical pixels; the node is placed in logical ones.
        let size = computed.size() * computed.inverse_scale_factor();
        if size.x <= 0.0 {
            // Not laid out yet.
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        let first = *label.first_width.get_or_insert(size.x);
        let shown = hint.shown();
        visibility.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
        if !shown {
            continue;
        }
        let countdown = hint.look == Look::Countdown;
        // An on-screen text is drawn in its colour, not faded or blinking
        // (`HintTile.Draw` uses `MainColor` there); a countdown fades.
        let opacity = if hint.draw_on_ui && !countdown { 1.0 } else { hint.state.opacity };
        color.0 = Color::srgba(1.0, 1.0, 1.0, opacity);
        // The point of the text that sits on the hint's place: its top left,
        // or for a countdown the middle of what it first said.
        let pivot = if countdown { Vec2::new(first / 2.0, size.y / 2.0) } else { Vec2::ZERO };
        let pulse = if countdown { hint.state.size } else { 1.0 };
        let (at, scale, angle) = if hint.draw_on_ui {
            let Some(here) = projector.point(view.at) else {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            };
            let k = hint.scale * pulse * if countdown { ui } else { 1.0 };
            (here, k, -view.direction)
        } else {
            let Some((here, per_unit, angle)) = projector.at(view.at, view.direction) else {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            };
            (here, hint.scale * pulse * per_unit / PX_PER_UNIT, angle)
        };
        // The node turns and scales about its middle: put the middle where
        // the pivot's offset from it lands on `at`.
        let centre = at - Vec2::from_angle(angle).rotate((pivot - size / 2.0) * scale);
        node.left = Val::Px(centre.x - size.x / 2.0);
        node.top = Val::Px(centre.y - size.y / 2.0);
        transform.scale = Vec2::splat(scale);
        transform.rotation = Rot2::radians(angle);
    }
}

/// An on-screen sprite: at the hint's place on the screen, a fixed size
/// (`Drawer.UIDisplayUnitRatio`), turned by the hint's direction, showing
/// the animation's frame.
#[allow(clippy::type_complexity)]
fn place_screen_sprites(
    hints: Query<&Hint>,
    cameras: Query<(&Camera, &Transform), With<MainCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sprites: Query<(&View, &ScreenSprite, &mut ImageNode, &mut Node, &mut UiTransform, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let projector = Projector { camera, transform: GlobalTransform::from(*camera_transform) };
    let ui = windows.iter().next().map_or(1.0, |w| ui_scale(canvas_size(Vec2::new(w.width(), w.height()))));
    for (view, sprite, mut image, mut node, mut transform, mut visibility) in &mut sprites {
        let Ok(hint) = hints.get(view.hint) else {
            continue;
        };
        let here = if hint.shown() { projector.point(view.at) } else { None };
        let Some(here) = here else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        let size = sprite.frame * hint.scale * hint.state.size * ui;
        node.left = Val::Px(here.x - size.x / 2.0);
        node.top = Val::Px(here.y - size.y / 2.0);
        node.width = Val::Px(size.x);
        node.height = Val::Px(size.y);
        transform.rotation = Rot2::radians(-view.direction);
        image.color = Color::srgba(1.0, 1.0, 1.0, hint.state.opacity);
        let frame = hint.frames.as_ref().map_or(0, Frames::current).min(sprite.frames - 1) as f32;
        image.rect = Some(Rect::new(frame * sprite.frame.x, 0.0, (frame + 1.0) * sprite.frame.x, sprite.frame.y));
    }
}
