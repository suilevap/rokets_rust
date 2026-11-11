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
        app.add_plugins(MaterialPlugin::<OverAllMaterial>::default())
            .add_systems(OnEnter(GameState::Playing), spawn_glow)
            .add_systems(Update, adapt.run_if(in_state(GameState::Playing)));
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
            base_color: tint(1.0),
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

/// `Color.White × rate`, premultiplied: added, it gives the light × rate².
fn tint(rate: f32) -> Color {
    Color::linear_rgba(rate, rate, rate, rate)
}

/// The rate the spot under the camera calls for (`GetLuminocity`).
pub fn target_rate(brightness: f32) -> f32 {
    1.0 - (brightness / DARK).min(1.0)
}

fn adapt(
    time: Res<Time>,
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
            material.base.base_color = tint(glow.rate);
        }
    }
}
