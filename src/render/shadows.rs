//! Gradient-sprite shadows from XNA's `DynamicShadow`: an ambient contact
//! blob and one/two directional gradients, with a smooth lamp hand-over.
//! All objects batch into two meshes/materials; a stationary scene does not
//! rebuild them. Ordinary depth testing hides the shadows behind walls.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::camera::MainCamera;
use super::layers::Layer;
use super::lighting_effects::LightingEffects;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::light::effects::{
    ShadowBlend, ShadowLights, ShadowPose, ShadowQuad, directional_shadow,
};

#[derive(Component, Clone, Copy, Debug)]
pub struct ShadowCaster {
    pub size: Vec2,
}

#[derive(Resource)]
struct Shadows {
    lights: ShadowLights,
    blends: HashMap<Entity, ShadowBlend>,
    poses: HashMap<Entity, ShadowPose>,
    directional: Handle<Mesh>,
    ambient: Handle<Mesh>,
    fading: bool,
    anchor: Vec2,
}

#[derive(Component)]
struct ShadowBatch;

pub struct ShadowsPlugin;

impl Plugin for ShadowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), prepare_shadows)
            .add_systems(
                PostUpdate,
                update_shadows
                    .after(TransformSystems::Propagate)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn shadow_mesh(quads: &[ShadowQuad], anchor: Vec2) -> Mesh {
    // Keep an invisible degenerate primitive for an empty batch: Bevy's
    // mesh allocator otherwise warns about the zero-length vertex buffer.
    let empty = [ShadowQuad {
        positions: [[0.0; 2]; 4],
        alpha: 0.0,
    }];
    let quads = if quads.is_empty() { &empty } else { quads };
    let mut positions = Vec::with_capacity(quads.len() * 4);
    let mut uvs = Vec::with_capacity(quads.len() * 4);
    let mut colors = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    for q in quads {
        let base = positions.len() as u32;
        positions.extend(
            q.positions
                .map(|[x, y]| [x - anchor.x, y - anchor.y, Layer::Shadow.z()]),
        );
        uvs.extend([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        colors.extend([[1.0, 1.0, 1.0, q.alpha]; 4]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn prepare_shadows(
    mut commands: Commands,
    map: Res<LoadedMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(gradient), Some(blob)) = (
        map.sprites.get("Gradient4Pixel"),
        map.sprites.get("Gradient4PixelUniform"),
    ) else {
        return;
    };
    let mut spawn = |name, texture: Handle<Image>| {
        let mesh = meshes.add(shadow_mesh(&[], Vec2::ZERO));
        commands.spawn((
            Name::new(name),
            MapEntity,
            ShadowBatch,
            NoFrustumCulling,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::BLACK,
                base_color_texture: Some(texture),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                depth_bias: Layer::Shadow.depth_bias(),
                ..default()
            })),
            Transform::default(),
        ));
        mesh
    };
    let directional = spawn("Directional shadows", gradient.image.clone());
    let ambient = spawn("Contact shadows", blob.image.clone());
    commands.insert_resource(Shadows {
        lights: ShadowLights::bake(&map.grid, &map.light.lamps),
        blends: HashMap::new(),
        poses: HashMap::new(),
        directional,
        ambient,
        fading: false,
        anchor: Vec2::ZERO,
    });
}

fn pose(caster: &ShadowCaster, transform: &GlobalTransform) -> ShadowPose {
    let (scale, rotation, translation) = transform.to_scale_rotation_translation();
    ShadowPose {
        position: [translation.x, translation.y],
        size: (caster.size * scale.truncate().abs()).to_array(),
        angle: rotation.to_euler(EulerRot::XYZ).2,
    }
}

fn ambient_quad(pose: ShadowPose) -> ShadowQuad {
    let (s, c) = pose.angle.sin_cos();
    let positions = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]].map(|[x, y]| {
        let (x, y) = (x * pose.size[0] * 2.2, y * pose.size[1] * 2.2);
        [
            pose.position[0] + x * c - y * s,
            pose.position[1] + x * s + y * c,
        ]
    });
    ShadowQuad {
        positions,
        alpha: 0.25,
    }
}

type Casters<'w, 's> =
    Query<'w, 's, (Entity, &'static ShadowCaster, &'static GlobalTransform), Without<ShadowBatch>>;
type Batches<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        &'static mut Transform,
        &'static mut GlobalTransform,
    ),
    With<ShadowBatch>,
>;

fn update_shadows(
    time: Res<Time>,
    map: Res<LoadedMap>,
    settings: Res<LightingEffects>,
    state: Option<ResMut<Shadows>>,
    (casters, mut batches, cameras): (Casters, Batches, Query<&MainCamera>),
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mut state) = state else {
        return;
    };
    if settings.is_changed() {
        for (mut v, _, _) in &mut batches {
            *v = if settings.shadows {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !settings.shadows {
        return;
    }
    let count = state.blends.len();
    let anchor = cameras
        .iter()
        .next()
        .map_or(Vec2::ZERO, |camera| camera.focus);
    state.blends.retain(|id, _| casters.get(*id).is_ok());
    state.poses.retain(|id, _| casters.get(*id).is_ok());
    let mut changed = state.fading
        || settings.is_changed()
        || count != state.blends.len()
        || anchor.distance_squared(state.anchor) > 50.0 * 50.0;
    state.fading = false;
    for (id, caster, transform) in &casters {
        let (x, y) = map.cell.to_cells(transform.translation().truncate());
        let target = state.lights.at(x, y);
        let pose = pose(caster, transform);
        changed |= state.poses.insert(id, pose) != Some(pose);
        let blend = state.blends.entry(id).or_default();
        state.fading |= blend.update(target, time.delta_secs());
    }
    if !(changed || state.fading) {
        return;
    }
    let mut directional = Vec::new();
    let mut ambient = Vec::new();
    for (id, _, _) in &casters {
        let pose = state.poses[&id];
        let (x, y) = map.cell.to_cells(Vec2::from(pose.position));
        let distance = map
            .grid
            .distance_to_wall(x.floor() as i32, y.floor() as i32)
            * map.cell.0;
        if let Some(blend) = state.blends.get(&id) {
            for &(lamp, weight) in blend.weights() {
                directional.extend(directional_shadow(
                    pose,
                    &map.light.lamps[lamp],
                    map.cell.0,
                    distance,
                    weight,
                ));
            }
        }
        ambient.push(ambient_quad(pose));
    }
    if let Some(mut mesh) = meshes.get_mut(&state.directional) {
        *mesh = shadow_mesh(&directional, anchor);
    }
    if let Some(mut mesh) = meshes.get_mut(&state.ambient) {
        *mesh = shadow_mesh(&ambient, anchor);
    }
    // Keep the batch's sorting origin near the view. Vertices are relative
    // to it, so the shared meshes sort between pads and bodies even far from
    // world zero. These root transforms are updated after propagation.
    state.anchor = anchor;
    for (_, mut transform, mut global) in &mut batches {
        *transform = Transform::from_translation(anchor.extend(0.0));
        *global = GlobalTransform::from(*transform);
    }
}

#[cfg(test)]
#[path = "shadows_tests.rs"]
mod tests;
