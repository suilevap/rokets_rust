//! The speed trail of a car: a pale streak behind it while it is fast, and
//! the lines that fly across the screen when the player's car is at its top
//! speed.
//!
//! C# origin: `Entities/Components/SpeedTrail.cs` (`Add SpeedTrail, and
//! ambient shadow for entity`, 2013-10-18, the streak; `Particles: Max
//! Speed UI particle effect`, 2013-10-31, the lines; `Speed trail effect
//! uses less particles(/4)`, 2013-12-23), with `ParticleSystemManager.
//! CreateSpeedTrails` and `_speedTrailStyle`. `EntitiesManager.CreateCar`
//! gives it to every `Car` (the `Players` cars, the player's and the AI's,
//! and their clones; not a `SimpleCars` car) of a map whose `LevelEffect`
//! is `None`, which is most of the campaign (`Settings.
//! EnableSpeedTrailForCamera`, a constant, `true`). The snow maps have
//! their own screen effects instead.
//!
//! - **The streak** (`Draw`): the `Gradient4Pixel` sprite the car's shadow
//!   is made of, white this time, drawn under the car (`CarBase + 0.1`:
//!   under the shadows and the pads, over the floor) from its middle
//!   backwards, the way it is *going* (not the way it faces). Once the car
//!   goes faster than 2.5 radii a second (`length = speed * 0.2` over half a
//!   radius), it is `speed * 0.2 + radius` long (times a third of the car's
//!   width per unit), `0.5 × speed / max speed` opaque. As wide as the car's
//!   sprite is long where it is seen end-on to the way it goes, and as
//!   narrow as the sprite is wide side-on (`1 + (1 - |cos|) × (length /
//!   width - 1)` of the base width, as the shadow's width), then each frame
//!   between half and the whole of that: it flickers.
//! - **The lines** (`Update`): while the stick of a car held for the
//!   player's devices (`Control.LocalDevice`) is pushed and it goes at its
//!   top speed (`SpeedAmountNormalized` over 0.93, the square of its speed
//!   over its maximum's), one tick in four (60 a second) makes a streak of
//!   [`SPEED_LINE`]: in the interface's pool, a screen's width either way
//!   of a point 512 pixels ahead of the car (its velocity over the maximum
//!   speed times 512), flying the other way at 1024 pixels a second, growing
//!   to 196 × 16 times the three-pixel sprite as it fades. The pool is the
//!   snow's: 64 slots in the effect canvas (`render::effect_canvas`).
//!
//! `SpeedTrailParticleStart` and `…End` (`DepthConst`, -4) are the height of
//! the interface's particles, the snow's too (`CreateSnowUI`), not of
//! anything here: they say the particles are drawn over everything else.
//! The camera's own speed lines (`Camera2DOperator.SpeedTrail`, the camera
//! lagging behind its focus) use the same `CreateSpeedTrails` but are the
//! camera's, on every map; the port does not have them. The car's smoke and
//! tyre marks (`smoke`) are other effects; they draw under (the wheel
//! streaks) or over (the exhaust) this one and do not read its numbers.
//!
//! Not ported:
//! - `World.Pause` (the flicker stopped and the width lost its end-on
//!   compensation while paused): the port has no pause. `World.TimeScale`
//!   does not apply either: the streak is made of the car's speed, which
//!   slow motion has already scaled.
//! - `RandomHelper`'s *fake* random numbers (a table drawn in turn):
//!   `ViewRng` is a plain generator, the same distribution.
//! - The update tick: C# rolled the lines' chance in 1/60 s steps; a frame
//!   rolls for the steps it spans ([`line_spawns`]), so the rate is the same
//!   at any frame rate.
//! - `GetFakePosition` (a network entity's smoothed position): the car's
//!   drawn position is used, which is what C# drew the car at.
//! - `ControlableEntity` (the `as` cast): only cars have the component.
//! - `Entity.Speed` clamping: the physics keeps a car at its top speed; the
//!   opacity is held to 1 all the same.
//! - The `DrawTrail`'s unused width `w` and the commented-out
//!   `DrawShadow` call.
//! - The speed lines of a simple car or any car without the component
//!   (they never had any), and the linear-light blending of both effects
//!   (see [`blend_alpha`]): the alpha is matched to what XNA's sRGB blend
//!   gave over a middling picture.

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use super::control::LocalDevice;
use super::{Car, CarRadius};
use crate::content::map::LevelEffect;
use crate::controls::PlayerIntent;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::SPEED_LINE;
use crate::particles::Pool;
use crate::render::effect_canvas::{Canvas, CanvasMade, MIDDLING_PICTURE, blend_alpha};
use crate::render::layers::Layer;
use crate::render::snow::{SCREEN_POOL, TICKS_PER_SECOND, show_pool};
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

/// The texture the streak is (and the car's shadow).
const STREAK: &str = "Gradient4Pixel";
/// `_gradientTextureBlackProportion`: the gradient has 2 opaque pixels of 4,
/// so it is drawn 1.75 times as wide as the streak.
const OPAQUE_PROPORTION: f32 = 1.75;
/// The streak's alpha at top speed (`speed / MaxSpeed * 0.5f`).
const MAX_ALPHA: f32 = 0.5;
/// The streak's length per unit of speed (`Speed.Length() * 0.2f`).
const LENGTH_PER_SPEED: f32 = 0.2;
/// `SpeedAmountNormalized` over this: the car is at its top speed.
pub const TOP_SPEED_AMOUNT: f32 = 0.93;
/// The chance of a line at each tick (`RandomEvent(0.25f)`).
pub const LINE_CHANCE: f32 = 0.25;
/// How far ahead of the car, in pixels, its lines start, at its top speed
/// (`CreateSpeedTrails`' `l`).
pub const LINE_LEAD: f32 = 512.0;
/// The texture the lines are.
pub const LINE_TEXTURE: &str = "Particles\\3Pixel";

/// Whether a level's cars have the speed trail: `EntitiesManager.CreateCar`
/// asks for `LevelEffect` `None`.
pub fn shows(effect: LevelEffect) -> bool {
    effect == LevelEffect::None
}

/// The sprites the lines need, to load with the map.
pub fn textures(effect: LevelEffect) -> Vec<&'static str> {
    if shows(effect) { vec![LINE_TEXTURE] } else { Vec::new() }
}

/// A car with the speed trail; `size` is its sprite (length, width) in
/// world units, as `CastsShadow`'s.
#[derive(Component, Debug, Clone, Copy)]
pub struct SpeedTrail {
    pub size: Vec2,
}

/// How the streak lies: the way it points, the size of one texel of the
/// 4 × 4 gradient (along it, across it) in world units and its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrailShape {
    /// World angle, away from the way the car goes.
    pub dir: f32,
    pub texel: Vec2,
    pub alpha: f32,
}

/// `SpeedTrail.Draw` and `DrawTrail` for a car going at `velocity` (world
/// units a second) whose top speed is `max_speed`, `radius` big, turned
/// `angle`, its sprite `size` (length, width); `roll` (0 to 1, random)
/// makes the width flicker. `None` while it is too slow.
pub fn trail_shape(velocity: Vec2, max_speed: f32, radius: f32, angle: f32, size: Vec2, roll: f32) -> Option<TrailShape> {
    let speed = velocity.length();
    let length = speed * LENGTH_PER_SPEED;
    if length <= radius / 2.0 || max_speed <= 0.0 {
        return None;
    }
    let dir = velocity.to_angle() + std::f32::consts::PI;
    let (long, wide) = (size.x, size.y);
    // The width side-on grows to the sprite's length over its width.
    let amount90 = (dir - angle).cos().abs();
    let across = 1.0 + (1.0 - amount90) * (long / wide - 1.0);
    let flicker = 0.5 + 0.5 * roll.clamp(0.0, 1.0);
    Some(TrailShape {
        dir,
        // `_baseScale`: the width over 4 texels, × 4/3 (the origin is a
        // quarter in), times the length; across, × 1.75 for the opaque part.
        texel: Vec2::new(wide / 3.0 * (length + radius), wide / 4.0 * OPAQUE_PROPORTION * across * flicker),
        alpha: (speed / max_speed * MAX_ALPHA).min(1.0),
    })
}

/// Whether the car makes speed lines now (`SpeedTrail.Update`): a local
/// device, the stick pushed, and the square of its speed over its maximum's
/// (`SpeedAmountNormalized`, held to 1) over [`TOP_SPEED_AMOUNT`].
pub fn at_top_speed(local: bool, stick: Vec2, velocity: Vec2, max_speed: f32) -> bool {
    let amount = (velocity.length_squared() / (max_speed * max_speed)).min(1.0);
    local && stick != Vec2::ZERO && max_speed > 0.0 && amount > TOP_SPEED_AMOUNT
}

/// How many lines a frame of `dt` seconds starts for one car at its top
/// speed: each tick it spans has the chance [`LINE_CHANCE`], `roll` (0 to
/// 1, random) settling the fraction.
pub fn line_spawns(dt: f32, roll: f32) -> usize {
    ((LINE_CHANCE * dt * TICKS_PER_SECOND) + roll).floor().max(0.0) as usize
}

/// Where a line starts and which way it flies (`CreateSpeedTrails`), in the
/// pool's axes as the snow's ([`crate::render::snow::screen_start`]): the
/// car's `velocity` over its `max_speed` is the point ahead of it, `spread`
/// a random vector of parts between -1 and 1, `canvas` the screen's size.
pub fn line_start(velocity: Vec2, max_speed: f32, spread: Vec2, canvas: Vec2) -> (Vec2, f32) {
    let ahead = velocity / max_speed;
    (ahead * LINE_LEAD + spread * canvas, ahead.to_angle() + std::f32::consts::PI)
}

/// What the effect's randomness draws from: the generator for what is only
/// seen.
#[derive(Resource)]
struct ViewRng(SmallRng);

/// The pool of the screen's lines.
#[derive(Resource)]
struct SpeedLines {
    pool: Pool,
}

/// One of the pool's slots, drawn.
#[derive(Component)]
struct Line;

/// The streak of a car.
#[derive(Component, Debug)]
struct Trail {
    caster: Entity,
}

pub struct SpeedTrailPlugin;

impl Plugin for SpeedTrailPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ViewRng(SmallRng::seed_from_u64(53)))
            .add_systems(OnEnter(GameState::Playing), start_lines.after(CanvasMade))
            .add_systems(
                Update,
                (
                    (spawn_trails, show_trails).chain(),
                    (spawn_lines, show_lines).chain().run_if(resource_exists::<SpeedLines>),
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn start_lines(mut commands: Commands, canvas: Option<Res<Canvas>>, map: Res<LoadedMap>) {
    commands.remove_resource::<SpeedLines>();
    let Some(canvas) = canvas.filter(|_| shows(map.data.level_effect)) else {
        return;
    };
    commands.insert_resource(SpeedLines { pool: Pool::new(SCREEN_POOL) });
    commands.entity(canvas.entity).with_children(|canvas| {
        for _ in 0..SCREEN_POOL {
            canvas.spawn((
                Name::new("Speed line"),
                Line,
                MapEntity,
                Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), ..default() },
                ImageNode::default(),
                UiTransform::IDENTITY,
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
        }
    });
}

fn spawn_trails(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cars: Query<Entity, Added<SpeedTrail>>,
) {
    let Some(streak) = map.sprites.get(STREAK) else {
        return;
    };
    let tint = SpriteTint { color: [1.0, 1.0, 1.0, 0.0], lit: true };
    for caster in &cars {
        commands.spawn((
            Name::new("Speed trail"),
            MapEntity,
            Trail { caster },
            // Culled for good by the frustum test, as the bounds of a quad
            // that is moved and stretched every frame, and a quad a car is cheap.
            bevy::camera::visibility::NoFrustumCulling,
            sprite_bundle(streak, 1.0, Layer::Trail, tint, &quad, &mut materials),
            Visibility::Hidden,
        ));
    }
}

#[allow(clippy::type_complexity)]
fn show_trails(
    mut rng: ResMut<ViewRng>,
    cars: Query<(&Transform, &LinearVelocity, &Car, &CarRadius, &SpeedTrail), Without<Trail>>,
    mut trails: Query<(Entity, &Trail, &mut Transform, &mut SpriteTint, &mut Visibility)>,
    mut commands: Commands,
) {
    for (entity, trail, mut transform, mut tint, mut visibility) in &mut trails {
        // The car is gone, or a wreck (`Car` is taken off it).
        let Ok((car_at, velocity, car, radius, speed_trail)) = cars.get(trail.caster) else {
            commands.entity(entity).despawn();
            continue;
        };
        let angle = car_at.rotation.to_euler(EulerRot::XYZ).2;
        let shape = trail_shape(velocity.0, car.params.max_speed, radius.0, angle, speed_trail.size, rng.0.random());
        let Some(shape) = shape else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        tint.color[3] = blend_alpha(shape.alpha, MIDDLING_PICTURE);
        // The texture's origin is its texel (1.5, 2): the quad's middle is
        // half a texel along the streak from the car.
        let middle = car_at.translation.truncate() + Vec2::from_angle(shape.dir) * shape.texel.x * 0.5;
        transform.translation = middle.extend(Layer::Trail.z());
        transform.rotation = Quat::from_rotation_z(shape.dir);
        transform.scale = (shape.texel * 4.0).extend(1.0);
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}

fn spawn_lines(
    time: Res<Time>,
    mut lines: ResMut<SpeedLines>,
    mut rng: ResMut<ViewRng>,
    canvas: Res<Canvas>,
    cars: Query<(&Car, &PlayerIntent, &LinearVelocity, Has<LocalDevice>), With<SpeedTrail>>,
) {
    let dt = time.delta_secs();
    lines.pool.step(dt);
    let rng = &mut rng.0;
    for (car, intent, velocity, local) in &cars {
        if !at_top_speed(local, intent.move_dir, velocity.0, car.params.max_speed) {
            continue;
        }
        for _ in 0..line_spawns(dt, rng.random()).min(SCREEN_POOL) {
            let spread = Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0);
            let (at, direction) = line_start(velocity.0, car.params.max_speed, spread, canvas.size);
            lines.pool.try_add(&SPEED_LINE, at, direction, direction);
        }
    }
}

type LineNode = (&'static mut Node, &'static mut ImageNode, &'static mut UiTransform, &'static mut Visibility);

fn show_lines(lines: Res<SpeedLines>, canvas: Res<Canvas>, map: Res<LoadedMap>, mut nodes: Query<LineNode, With<Line>>) {
    show_pool(&lines.pool, &canvas, &map, nodes.iter_mut());
}
