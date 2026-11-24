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
//! - The dynamic glass flashes (`CreateExplosionEffectUI`'s yellow flash
//!   through the glass in place of the plain one, `CreateLaserEffect`, the
//!   `LaserCollectorButton` pulse, `GetDirtyGlassLineView`): they need the
//!   particle and the laser code to hand their positions on, a piece of
//!   their own. The port draws no `CreateExplosionEffectUI` flash at all
//!   yet (see `explosion`).
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
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::camera::ViewRotation;
use crate::render::layers::ADDITIVE_DEPTH_BIAS;

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
            .add_plugins(MaterialPlugin::<DirtyLensMaterial>::default())
            .add_systems(OnEnter(GameState::Playing), spawn_lens)
            .add_systems(
                Update,
                fit_glass
                    .run_if(in_state(GameState::Playing))
                    .run_if(resource_exists::<LoadedMap>),
            );
    }
}

fn spawn_lens(
    mut commands: Commands,
    map: Res<LoadedMap>,
    boost: Res<LensBoost>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DirtyLensMaterial>>,
) {
    if !shows(map.data.level_effect) || map.data.lights.items.is_empty() {
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
            LensLamp,
        ));
    }
}

/// A lamp's blob.
#[derive(Component)]
struct LensLamp;

/// Keeps the glass's window to the screen's shape and turn.
fn fit_glass(
    windows: Query<&Window, With<PrimaryWindow>>,
    view: Res<ViewRotation>,
    lamps: Query<&MeshMaterial3d<DirtyLensMaterial>, With<LensLamp>>,
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
    for handle in &lamps {
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
