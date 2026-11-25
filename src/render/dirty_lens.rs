//! The dirty lens: the camera's smudged glass, lit up where the map's lamps
//! are.
//!
//! C# origin: `DirtyLensEffectDualTexture` (2013, `Exp: dirty lens effect`;
//! it moved to a `DualTextureEffect` in `Dirty Lens use DualTextureEffect`),
//! made for a plain map by `MapEffectFactory` (`LevelEffect` `None`; the
//! snow maps have `FrozenGlassEffect` instead). Two textures meet in
//! `DualTextureEffect`:
//!
//! - the lens light map, a render target of the whole map at a quarter of
//!   its size (`DirtyLensEffectDesireLightMapM`), made once from
//!   `LightManager.StaticLights`: each lamp a `flash2` blob added in the
//!   lamp's colour, its radius the lamp's `R` held between 4 and 16
//!   (`DirtyLensEffectMinLightR`, `DirtyLensEffectMaxLightR`) times 0.8
//!   (`DirtyLensEffectLightScale`);
//! - `DirtyGlass`, bokeh and dust, fixed to the screen.
//!
//! The effect draws a screen quad with the light map at the world under it
//! and the glass at the screen, `light × glass × 2` (the shader's
//! modulate-2x) at an alpha of `DirtyLensEffectAmount` (0.6), added to the
//! picture (`RendererConst.DirtyLensUI`, `BlendState.Additive`, before the
//! interface). So the glass shows only near lamps, tinted by them: yellow
//! and green lamps give LetItSnowDay its green haze.
//!
//! Here each lamp is a quad in the world, drawn over everything, whose
//! shader multiplies the blob by the glass at the screen position; added
//! blobs sum as the light map did, and there is no map-sized texture, no
//! second camera and no full-screen pass, so it runs on WebGL2 as it is.
//!
//! The same effect flashes the glass (2015-06 `Exp: dynamic dirty glass
//! effect`, `Adjust explosion effect`; the laser's in 2015-10 `Add effect
//! when enemy laser collides`). `DirtyLensEffectDualTexture` keeps a second
//! `DualTextureEffect` of `flash2` and the glass with vertex colours on,
//! whose sprites the particle system adds through `DirtyGlassFlashView`
//! (`Draw` turns a particle's world position and `Scale` into a screen quad,
//! `flash2`'s 32 pixels times `Scale` in world units) and the effect draws
//! with the lens, over the picture and under the interface. A flash is a
//! blob of one colour that fades as it shrinks; the colour is premultiplied
//! (`Color.Yellow * (1 - t)`), so it dims in the colour and in the blend's
//! alpha both, and the glass shows through it where it is. They are made
//! only while the effect is live (`IsDirtyGlassViewsValid`: a map with
//! `LevelEffect` `None`, see [`shows`]; a snow map has the frost instead):
//!
//! - an explosion (`CreateBigBoom` through `CreateExplosionEffectUI`, from
//!   `WeaponHelper.CreateExplosion` when the blast is within the view by
//!   twice its range, `Camera.IsInView`): a yellow blob of 48 units
//!   shrinking to 32 over a second ([`Flash::Explosion`]); the teleport
//!   makes the same where a car or box turns up again after its half
//!   second (`Teleport.TeleportActivate`);
//! - a beam across the player's car (`CreateLaserEffect`, from
//!   `LaserRenderer.Render`): every frame a beam leg ends on a car held for
//!   the player's devices, one in four, a red blob of 24 units shrinking to
//!   16 over a quarter second, a quarter of the way along that leg from
//!   where its ray starts, shaken by up to half a unit ([`Flash::Laser`]).
//!
//! Explosions are read here from `explosion::Explosion`; the others come as
//! a [`GlassFlash`] message from the system that knows (`weapon/laser`,
//! `teleport`).
//!
//! Not ported:
//! - `DirtyLensEffectAlgorithm` 0 and 1 (the glass multiplied into the
//!   picture, the light map drawn through a render target), `Settings.
//!   EnableDirtyLensEffect` and the debug key that turns the effect off
//!   (`RenderSettings.Phase.DirtyLensEffect`, `Settings.DrawEnvironment`):
//!   constants and debug switches, the shipped game uses algorithm 2.
//! - The light map's 8-bit saturation: where blobs overlap, C# held the sum
//!   at 1 before the glass; here the sum goes on past it, so overlapping
//!   lamps are a little brighter. Outside the map C# stretched the edge
//!   texels of the light map; here a blob stops where it ends.
//! - `LaserCollectorButton`'s pulse (`DrawLight`: a white flash of 16 to
//!   21 units, brightness 0.25 to 0.75, breathing twice a second behind
//!   an active button): the button is not ported (see `button`); when it is,
//!   it hands the lens a [`GlassFlash`] each frame it is lit.
//! - `GetDirtyGlassLineView` and `DirtyLensEffectDualTexture`'s line
//!   renderer (`3Pixel`, 2015-06 `Exp: line dirty glass view`): drawn each
//!   frame but nothing ever adds a line to it; the one caller in
//!   `LaserRenderer` was commented out in the same experiment.
//! - `CreateExplosionEffectUI`'s other branch, the plain flash a map without
//!   the lens gets instead (`_explosionUiStyle`: `flashTransparent`, yellow
//!   half bright, 4 s, in screen space): the port has no screen-space
//!   particles yet, so a snow map shows no flash at all.
//! - The flashes' place in `_poolAdditive`: a pool of 64 shared with the
//!   other additive particles, a flash dropped when it is full. Here the
//!   flashes have their own entities, [`MAX_FLASHES`] of them.
//! - `Settings.HighPerformanceDevice` (a constant `true` on every platform
//!   the game ships for) and `CreateBigBoomOld` (no caller).
//! - `ChangeAmount` (the `ChangeMapEffectAmount` action): the dual-texture
//!   effect keeps it and never reads it.
//! - Lost render targets (`ContentLost`) and the aspect fix by
//!   `DualTextureEffectRender`'s `Viewport.AspectRatio`: a texture of this
//!   port's is never lost; the glass keeps its aspect as C# did, see
//!   [`glass_window`].
//!
//! The light is added in linear colour here, in sRGB numbers in C# (XNA has
//! no gamma): [`DEFAULT_LENS_BOOST`] takes up the difference, as
//! `additive_light::DEFAULT_GLOW_BOOST` does for the glow.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::image::ImageLoaderSettings;
use bevy::math::Vec4;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::window::PrimaryWindow;

use crate::content::map::LevelEffect;
use crate::content::xna::XColor;
use crate::explosion::Explosion;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::camera::{MainCamera, ViewRotation};
use crate::render::layers::ADDITIVE_DEPTH_BIAS;
use crate::units::PX_PER_UNIT;
use crate::weapon::aim_help::in_view;

/// A lamp's radius, in world units, is held between these (`DirtyLensEffect
/// MinLightR`, `MaxLightR`) …
pub const MIN_RADIUS: f32 = 4.0;
pub const MAX_RADIUS: f32 = 16.0;
/// … and then shrunk by this (`DirtyLensEffectLightScale`).
pub const LIGHT_SCALE: f32 = 0.8;
/// The alpha the effect draws at (`DirtyLensEffectAmount`).
pub const AMOUNT: f32 = 0.6;
/// The picture the glass was made for, `Settings.ViewWidth / ViewHeight`;
/// a window of another shape shows the glass cropped, not stretched.
pub const GLASS_ASPECT: f32 = 800.0 / 480.0;
/// What the linear sum is multiplied by to look as the sRGB one did (fitted
/// to C# screenshots of LetItSnowDay: the lens added about as much light).
pub const DEFAULT_LENS_BOOST: f32 = 0.9;

/// How many flashes at most are alive (`MaxParticleCountAdditive ×
/// CountRate`, the pool they were in).
pub const MAX_FLASHES: usize = 64;
/// The blob's side in the C# game's pixels (`flash2`), which a flash's
/// `Scale` multiplies.
pub const BLOB_PX: f32 = 32.0;
/// A beam leg's chance a frame of flashing when it ends on the player's car
/// (`RandomEvent(0.25f)`).
pub const CROSSING_CHANCE: f32 = 0.25;
/// How far a laser flash is shaken from its place, in world units
/// (`NextVector2Fake() * 0.5f`: each axis up to this either way).
pub const CROSSING_SHAKE: f32 = 0.5;
/// How far from the view an explosion still flashes, in blast ranges
/// (`CreateExplosion`: `IsInView(position, range * 2)`).
pub const EXPLOSION_VIEW_RANGES: f32 = 2.0;

/// Whether a level has the dirty lens: a plain one, not a snow one
/// (`MapEffectFactory`).
pub fn shows(effect: LevelEffect) -> bool {
    effect == LevelEffect::None
}

/// The radius of a lamp's blob in world units, for a lamp of `r_cells`
/// cells on a map of `cell` world units per cell.
pub fn blob_radius(r_cells: f32, cell: f32) -> f32 {
    LIGHT_SCALE * (r_cells * cell).clamp(MIN_RADIUS, MAX_RADIUS)
}

/// The colour the lamp's blob is added in: its colour times its alpha
/// (`LightManager.AddLight` premultiplied it), as numbers from 0 to 1.
pub fn blob_color(color: XColor) -> [f32; 3] {
    let a = color.a as f32 / 255.0;
    [color.r, color.g, color.b].map(|c| c as f32 / 255.0 * a)
}

/// What the colour sum is multiplied by before it is added: the shader's
/// modulate-2x, the effect's alpha for the colour and again as the blend's
/// source alpha, and `boost` for the missing gamma.
pub fn gain(boost: f32) -> f32 {
    2.0 * AMOUNT * AMOUNT * boost
}

/// The part of the glass the screen shows, as `(scale, offset)` on its
/// texture coordinates, for a view `aspect` (long side over short): the
/// glass was drawn for [`GLASS_ASPECT`], and a wider view crops its top and
/// bottom, a narrower one its sides, as `DualTextureEffectRender` did.
pub fn glass_window(aspect: f32) -> (Vec2, Vec2) {
    let (mut dx, mut dy) = (0.0, 0.0);
    if (aspect - GLASS_ASPECT).abs() > f32::EPSILON {
        if aspect > GLASS_ASPECT {
            dy = (1.0 - GLASS_ASPECT / aspect) / 2.0;
        } else {
            dx = (1.0 - aspect / GLASS_ASPECT) / 2.0;
        }
    }
    (Vec2::new(1.0 - 2.0 * dx, 1.0 - 2.0 * dy), Vec2::new(dx, dy))
}

/// What flashes the glass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flash {
    /// `_explosionDirtyGlassStyle`: a blast, or a car turning up from a
    /// teleport.
    Explosion,
    /// `_laserDirtyGlassStyle`: a beam across the player's car.
    Laser,
}

/// How a flash starts, ends and fades (a `ParticleStyle`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlashLook {
    /// The colour at the start (`BeginState.Color`), 0..1.
    pub color: [f32; 3],
    /// The `Scale` at the start and at the end, times [`BLOB_PX`] pixels.
    pub begin: f32,
    pub end: f32,
    /// Seconds.
    pub lifetime: f32,
}

impl Flash {
    pub const fn look(self) -> FlashLook {
        match self {
            Flash::Explosion => FlashLook {
                color: [1.0, 1.0, 0.0],
                begin: 48.0,
                end: 32.0,
                lifetime: 1.0,
            },
            Flash::Laser => FlashLook {
                color: [1.0, 0.0, 0.0],
                begin: 24.0,
                end: 16.0,
                lifetime: 0.25,
            },
        }
    }
}

/// A flash `age` seconds into its life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlashState {
    /// The blob's width, in world units.
    pub diameter: f32,
    /// How much of its colour is left, 1 to 0 (`GetState`: the begin
    /// colour times `1 - age / lifetime`, as the end colour is transparent).
    pub fade: f32,
}

/// The flash at `age`, or `None` once its life is over.
pub fn flash_state(flash: Flash, age: f32) -> Option<FlashState> {
    let look = flash.look();
    if age >= look.lifetime {
        return None;
    }
    let amount = (age / look.lifetime).max(0.0);
    let scale = look.begin + (look.end - look.begin) * amount;
    Some(FlashState {
        diameter: scale * BLOB_PX / PX_PER_UNIT,
        fade: 1.0 - amount,
    })
}

/// What a flash's colour is multiplied by before it is added, as
/// [`blob_color`]'s are: the begin colour times the fade twice (the vertex
/// colour is premultiplied, so it dims the colour and the blend's alpha),
/// and the gain.
pub fn flash_color(flash: Flash, fade: f32, boost: f32) -> [f32; 3] {
    flash.look().color.map(|c| c * fade * fade * gain(boost))
}

/// The radius a blast of `range` units counts as in view with: it flashes
/// the glass when its middle is within the picture, with this margin
/// (`IsInView(position, range * 2)`, see [`in_view`]).
pub fn explosion_radius(range: f32) -> f32 {
    range * EXPLOSION_VIEW_RANGES
}

/// The glass flashes here (a world position): sent by whatever makes the
/// flash that the lens does not read from an existing message.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct GlassFlash {
    pub at: Vec2,
    pub flash: Flash,
}

/// One lamp's blob: the light colour and the glass, summed on the picture.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct DirtyLensMaterial {
    /// The blob's colour times the gain; the alpha is not used (blending is
    /// premultiplied, and the shader returns alpha 0 to add).
    #[uniform(0)]
    pub color: LinearRgba,
    /// `xy` the scale and `zw` the offset of the glass window.
    #[uniform(0)]
    pub glass_window: Vec4,
    /// 1 when the view is turned a quarter (a portrait screen), so the
    /// glass turns with it.
    #[uniform(0)]
    pub turned: f32,
    #[texture(1)]
    #[sampler(2)]
    pub blob: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub glass: Handle<Image>,
}

impl Material for DirtyLensMaterial {
    fn fragment_shader() -> ShaderRef {
        AssetPath::from_path_buf(embedded_path!("dirty_lens.wesl"))
            .with_source("embedded")
            .into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn depth_bias(&self) -> f32 {
        // Sorted after the sprites, with the additive light.
        ADDITIVE_DEPTH_BIAS
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Over everything, walls included, as the screen quad was.
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// How strongly the lens is added: a factor on the sum (see
/// [`DEFAULT_LENS_BOOST`]).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct LensBoost(pub f32);

impl Default for LensBoost {
    fn default() -> Self {
        LensBoost(DEFAULT_LENS_BOOST)
    }
}

pub struct DirtyLensPlugin;

impl Plugin for DirtyLensPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "dirty_lens.wesl");
        app.init_resource::<LensBoost>()
            .add_message::<GlassFlash>()
            .add_plugins(MaterialPlugin::<DirtyLensMaterial>::default())
            .add_systems(OnEnter(GameState::Playing), spawn_lens)
            .add_systems(
                Update,
                (
                    start_flashes.run_if(resource_exists::<LensAssets>),
                    age_flashes,
                    fit_glass,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing))
                    .run_if(resource_exists::<LoadedMap>),
            );
    }
}

/// What the lens draws with, there while the map has the lens.
#[derive(Resource, Clone)]
struct LensAssets {
    quad: Handle<Mesh>,
    blob: Handle<Image>,
    glass: Handle<Image>,
}

fn spawn_lens(
    mut commands: Commands,
    map: Res<LoadedMap>,
    boost: Res<LensBoost>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DirtyLensMaterial>>,
) {
    if !shows(map.data.level_effect) {
        commands.remove_resource::<LensAssets>();
        return;
    }
    // The glass is read as the numbers it holds (as XNA did), not as sRGB
    // colour.
    let glass: Handle<Image> = server
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false)
        .load("Sprites/DirtyGlass.png");
    let blob: Handle<Image> = server.load("Sprites/Particles/flash2.png");
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    commands.insert_resource(LensAssets {
        quad: quad.clone(),
        blob: blob.clone(),
        glass: glass.clone(),
    });
    let gain = gain(boost.0);
    for (i, light) in map.data.lights.iter().enumerate() {
        let [r, g, b] = blob_color(light.color).map(|c| c * gain);
        let diameter = 2.0 * blob_radius(light.r, map.cell.0);
        let at = map.world(light.position.x, light.position.y);
        commands.spawn((
            Name::new(format!("Dirty lens lamp {i}")),
            MapEntity,
            Mesh3d(quad.clone()),
            MeshMaterial3d(materials.add(DirtyLensMaterial {
                color: LinearRgba::new(r, g, b, 0.0),
                glass_window: Vec4::new(1.0, 1.0, 0.0, 0.0),
                turned: 0.0,
                blob: blob.clone(),
                glass: glass.clone(),
            })),
            Transform::from_translation(at.extend(0.0)).with_scale(Vec3::new(diameter, diameter, 1.0)),
        ));
    }
}

/// A flash of the glass alive: how long it has.
#[derive(Component, Debug)]
struct FlashAge {
    flash: Flash,
    age: f32,
}

/// Starts the flashes asked for: blasts near the view (read here from
/// [`Explosion`], as `CreateBigBoom` was called by `CreateExplosion`) and
/// every [`GlassFlash`]; none while [`MAX_FLASHES`] are alive.
#[allow(clippy::too_many_arguments)]
fn start_flashes(
    mut commands: Commands,
    assets: Res<LensAssets>,
    boost: Res<LensBoost>,
    mut explosions: MessageReader<Explosion>,
    mut asked: MessageReader<GlassFlash>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    alive: Query<(), With<FlashAge>>,
    mut materials: ResMut<Assets<DirtyLensMaterial>>,
) {
    let mut room = MAX_FLASHES.saturating_sub(alive.iter().count());
    let blasts = explosions.read().filter_map(|ex| {
        let (camera, transform) = cameras.single().ok()?;
        in_view(camera, transform, ex.at, explosion_radius(ex.blast.range)).then_some(GlassFlash {
            at: ex.at,
            flash: Flash::Explosion,
        })
    });
    for GlassFlash { at, flash } in blasts.chain(asked.read().copied()) {
        if room == 0 {
            break;
        }
        room -= 1;
        let [r, g, b] = flash_color(flash, 1.0, boost.0);
        commands.spawn((
            Name::new("Glass flash"),
            MapEntity,
            FlashAge { flash, age: 0.0 },
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(materials.add(DirtyLensMaterial {
                color: LinearRgba::new(r, g, b, 0.0),
                glass_window: Vec4::new(1.0, 1.0, 0.0, 0.0),
                turned: 0.0,
                blob: assets.blob.clone(),
                glass: assets.glass.clone(),
            })),
            Transform::from_translation(at.extend(0.0)),
        ));
    }
}

/// Ages the flashes and sizes and dims them to their moment; those past
/// their life go. (Game time: a paused game holds them, slow motion
/// slows them, as it did the particles.)
fn age_flashes(
    mut commands: Commands,
    time: Res<Time>,
    boost: Res<LensBoost>,
    mut flashes: Query<(Entity, &mut FlashAge, &mut Transform, &MeshMaterial3d<DirtyLensMaterial>)>,
    mut materials: ResMut<Assets<DirtyLensMaterial>>,
) {
    for (entity, mut flash, mut transform, material) in &mut flashes {
        flash.age += time.delta_secs();
        let Some(state) = flash_state(flash.flash, flash.age) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.scale = Vec3::new(state.diameter, state.diameter, 1.0);
        let [r, g, b] = flash_color(flash.flash, state.fade, boost.0);
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.color = LinearRgba::new(r, g, b, 0.0);
        }
    }
}

/// Keeps the glass's window to the screen's shape and turn.
fn fit_glass(
    windows: Query<&Window, With<PrimaryWindow>>,
    view: Res<ViewRotation>,
    lenses: Query<&MeshMaterial3d<DirtyLensMaterial>>,
    mut materials: ResMut<Assets<DirtyLensMaterial>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let (w, h) = (window.width(), window.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let turned = if view.angle.abs() > std::f32::consts::FRAC_PI_4 { 1.0 } else { 0.0 };
    let (scale, offset) = glass_window(w.max(h) / w.min(h));
    let window = Vec4::new(scale.x, scale.y, offset.x, offset.y);
    for handle in &lenses {
        if materials
            .get(&handle.0)
            .is_some_and(|m| m.glass_window != window || m.turned != turned)
            && let Some(mut material) = materials.get_mut(&handle.0)
        {
            material.glass_window = window;
            material.turned = turned;
        }
    }
}
