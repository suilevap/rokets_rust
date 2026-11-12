//! The additive (HDR) light: lamps with an `AdditiveRate` glow on top of
//! the picture, walls and cars included, more the darker the spot the
//! camera looks at, as an eye adapting to the dark.
//!
//! C# origin: `LightManager`'s second draw of its light (2014), the
//! additive target over the map in `RendererConst.IgnoreDepthWorldAdditive`
//! (additive blend, no depth test), tinted by `HdrHelper.GetLuminocity`:
//! how much additive light falls where the camera looks, `v`, gives
//! `1 − min(v / 0.2, 1)`, eased (`Hardastic`, 0.75 a second, from 1); the
//! tint `White × rate` with an additive blend adds the light times rate².

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::camera::MainCamera;
use crate::render::layers::{ADDITIVE_DEPTH_BIAS, LIGHT_Z};
use crate::render::light_layer::{light_quad, light_texture};

/// Below this much additive light under the camera, the glow starts to
/// show; in the dark it is full (`HdrHelper`).
const DARK: f32 = 0.2;
/// How much brighter the glow is added than its texture says, by default.
/// XNA added sRGB numbers (0.3 + 0.3 = 0.6); Bevy adds light in linear
/// colour (0.3 + 0.3 comes out about 0.42), so over a lit spot the glow
/// looked much weaker. Twice the linear light fits XNA's sums best on
/// average; three times errs on the bright side, letting the brightest
/// parts burn out, which reads closer to the original than too little.
pub const DEFAULT_GLOW_BOOST: f32 = 3.0;
/// The step of the `[` and `]` keys.
const BOOST_STEP: f32 = 0.25;
/// The most the keys turn it up to.
const MAX_BOOST: f32 = 10.0;

/// How strongly the additive light is added: a factor on its linear
/// light. Set with `--glow X` (`?glow=X` on the web), turned by `[` and
/// `]` while playing.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct GlowBoost(pub f32);

impl Default for GlowBoost {
    fn default() -> Self {
        GlowBoost(DEFAULT_GLOW_BOOST)
    }
}

/// How fast the eye adapts, per second (`Hardastic` K).
const ADAPT_RATE: f32 = 0.75;

/// A standard unlit material drawn over everything: no depth test, no
/// depth write.
pub type OverAllMaterial = ExtendedMaterial<StandardMaterial, OverAll>;

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct OverAll {}

impl MaterialExtension for OverAll {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// The glow's strength now: eased towards what the spot under the camera
/// calls for.
#[derive(Component)]
struct Glow {
    rate: f32,
}

pub struct AdditiveLightPlugin;

impl Plugin for AdditiveLightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlowBoost>()
            .add_plugins(MaterialPlugin::<OverAllMaterial>::default())
            .add_systems(OnEnter(GameState::Playing), spawn_glow)
            .add_systems(
                Update,
                (turn_boost, adapt)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn spawn_glow(
    mut commands: Commands,
    map: Res<LoadedMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<OverAllMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(additive) = &map.light.additive else {
        return;
    };
    let (mesh, transform) = light_quad(&map, LIGHT_Z);
    let material = ExtendedMaterial {
        base: StandardMaterial {
            base_color: tint(1.0, DEFAULT_GLOW_BOOST),
            base_color_texture: Some(images.add(light_texture(additive.image()))),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            depth_bias: ADDITIVE_DEPTH_BIAS,
            ..default()
        },
        extension: OverAll {},
    };
    commands.spawn((
        Name::new("Additive light"),
        MapEntity,
        Glow { rate: 1.0 },
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(material)),
        transform,
    ));
}

/// `Color.White × rate`, premultiplied: added, it gives the light × rate²
/// (times the boost).
fn tint(rate: f32, boost: f32) -> Color {
    let glow = rate * boost;
    Color::linear_rgba(glow, glow, glow, rate)
}

/// `[` and `]` turn the boost down and up, to try values while playing.
fn turn_boost(keys: Res<ButtonInput<KeyCode>>, mut boost: ResMut<GlowBoost>) {
    let step = if keys.just_pressed(KeyCode::BracketRight) {
        BOOST_STEP
    } else if keys.just_pressed(KeyCode::BracketLeft) {
        -BOOST_STEP
    } else {
        return;
    };
    boost.0 = (boost.0 + step).clamp(0.0, MAX_BOOST);
    info!("glow boost {:.2}", boost.0);
}

/// The rate the spot under the camera calls for (`GetLuminocity`).
pub fn target_rate(brightness: f32) -> f32 {
    1.0 - (brightness / DARK).min(1.0)
}

fn adapt(
    time: Res<Time>,
    boost: Res<GlowBoost>,
    map: Res<LoadedMap>,
    camera: Query<&MainCamera>,
    mut glows: Query<(&mut Glow, &MeshMaterial3d<OverAllMaterial>, &mut Visibility)>,
    mut materials: ResMut<Assets<OverAllMaterial>>,
) {
    let (Some(additive), Ok(camera)) = (&map.light.additive, camera.single()) else {
        return;
    };
    let (x, y) = map.cell.to_cells(camera.ground);
    let target = target_rate(additive.brightness_at(x, y));
    let dt = time.delta_secs();
    for (mut glow, handle, mut visibility) in &mut glows {
        // `Hardastic.Update`: eases by K per second, snaps when a frame is
        // longer than 1 / K.
        glow.rate = if dt * ADAPT_RATE > 1.0 {
            target
        } else {
            glow.rate + (target - glow.rate) * ADAPT_RATE * dt
        };
        let shown = if glow.rate > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base.base_color = tint(glow.rate, boost.0);
        }
    }
}
