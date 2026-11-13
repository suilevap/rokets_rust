//! XNA's `DirtyLensEffectDualTexture` and `World.ViewDarkCorners`.
//! The light circles are baked once; the GPU projects the screen into that
//! world map and modulates it with screen-fixed dirty glass (Modulate2X).
//! Vignetting and additive lens light share one fullscreen quad, before HUD.

use bevy::asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
    RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::effect_assets::EffectTextures;
use super::layers::SCREEN_EFFECT_DEPTH_BIAS;
use super::light_layer::light_texture;
use crate::content::map::LevelEffect;
use crate::game::{GameState, LoadedMap, MapEntity};

#[derive(Resource, Clone, Copy, Debug)]
pub struct LightingEffects {
    pub lens: f32,
    pub vignette: f32,
    pub shadows: bool,
}

impl Default for LightingEffects {
    fn default() -> Self {
        Self {
            lens: 0.6,
            vignette: 1.0,
            shadows: true,
        }
    }
}

impl LightingEffects {
    pub fn disabled() -> Self {
        Self {
            lens: 0.0,
            vignette: 0.0,
            shadows: false,
        }
    }
}

pub struct LightingEffectsPlugin;

impl Plugin for LightingEffectsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "lighting_effects.wesl");
        app.init_resource::<LightingEffects>()
            .add_plugins(MaterialPlugin::<LensMaterial>::default())
            .add_systems(OnEnter(GameState::Playing), spawn_overlay)
            .add_systems(Update, update_settings.run_if(in_state(GameState::Playing)));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct LensMaterial {
    /// Reciprocal world size, lens amount, vignette amount.
    #[uniform(0)]
    params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    light: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    dirt: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    corners: Handle<Image>,
}

impl Material for LensMaterial {
    fn vertex_shader() -> ShaderRef {
        AssetPath::from_path_buf(embedded_path!("lighting_effects.wesl"))
            .with_source("embedded")
            .into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn depth_bias(&self) -> f32 {
        SCREEN_EFFECT_DEPTH_BIAS
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
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = Some(CompareFunction::Always);
            depth.depth_write_enabled = Some(false);
        }
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                // (scene + lens) * vignette, retaining destination alpha.
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::SrcAlpha,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::Zero,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::Add,
                    },
                });
            }
        }
        Ok(())
    }
}

fn spawn_overlay(
    mut commands: Commands,
    map: Res<LoadedMap>,
    settings: Res<LightingEffects>,
    textures: Res<EffectTextures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LensMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let lens = &map.light.lens;
    let light = light_texture((
        lens.width,
        lens.height,
        lens.texels.iter().map(|c| [c[0], c[1], c[2]]).collect(),
    ));
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    let room = map.light.frame.room;
    commands.spawn((
        Name::new("Dirty lens and vignette"),
        MapEntity,
        NoFrustumCulling,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(LensMaterial {
            params: Vec4::new(
                1.0 / room[0].max(1.0),
                1.0 / room[1].max(1.0),
                if map.data.level_effect == LevelEffect::None {
                    settings.lens
                } else {
                    0.0
                },
                settings.vignette,
            ),
            light: images.add(light),
            dirt: textures.dirty_glass.clone(),
            corners: textures.dark_corners.clone(),
        })),
        Transform::default(),
        if settings.lens > 0.0 || settings.vignette > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    ));
}

fn update_settings(
    settings: Res<LightingEffects>,
    map: Res<LoadedMap>,
    mut overlays: Query<(&MeshMaterial3d<LensMaterial>, &mut Visibility)>,
    mut materials: ResMut<Assets<LensMaterial>>,
) {
    if !settings.is_changed() {
        return;
    }
    for (handle, mut visibility) in &mut overlays {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.params.z = if map.data.level_effect == LevelEffect::None {
                settings.lens.max(0.0)
            } else {
                0.0
            };
            material.params.w = settings.vignette.clamp(0.0, 1.0);
        }
        *visibility = if settings.lens > 0.0 || settings.vignette > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
