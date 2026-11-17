//! Draws the particle pool (`particles`): every frame, one mesh per
//! texture and layer, a quad per particle with its colour in the vertex
//! colours, lit by the light layer where it is, as the C# game drew its
//! particles in the world sprite batch.
//!
//! Particles under the cars' height (wheel streaks) go on
//! [`Layer::GroundParticles`], the rest (smoke) on [`Layer::AirParticles`].

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::CAR_BASE;
use crate::particles::{Particle, Pool};
use crate::render::layers::Layer;
use crate::units::px;

/// `ParticleSystemManager.MaxParticleCount × CountRate` on desktop.
const WORLD_POOL: usize = 128;

/// The world's particles. Effects add to it; it is cleared with the map.
#[derive(Resource)]
pub struct Particles(pub Pool);

impl Default for Particles {
    fn default() -> Self {
        Particles(Pool::new(WORLD_POOL))
    }
}

/// A batch mesh: one per texture and layer, rebuilt each frame.
#[derive(Component)]
struct Batch {
    texture: &'static str,
    layer: Layer,
}

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Particles>()
            .add_systems(OnEnter(GameState::Playing), clear)
            .add_systems(
                PostUpdate,
                (step, draw)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn clear(mut particles: ResMut<Particles>) {
    particles.0.clear();
}

fn step(time: Res<Time>, mut particles: ResMut<Particles>) {
    particles.0.step(time.delta_secs());
}

fn layer_of(p: &Particle) -> Layer {
    if p.state.height > CAR_BASE / 2.0 {
        Layer::GroundParticles
    } else {
        Layer::AirParticles
    }
}

#[derive(Default)]
struct Quads {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

fn draw(
    mut commands: Commands,
    map: Res<LoadedMap>,
    particles: Res<Particles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut batches: Query<(&Batch, &Mesh3d)>,
) {
    let mut quads: HashMap<(&'static str, Layer), Quads> = HashMap::new();
    for p in particles.0.iter() {
        let Some(sprite) = map.sprites.get(p.style.texture) else {
            continue;
        };
        let q = quads.entry((p.style.texture, layer_of(p))).or_default();
        let half = sprite.size * p.state.scale * px(1.0) / 2.0;
        let (x, y) = (Vec2::from_angle(p.view_direction) * half.x, Vec2::from_angle(p.view_direction).perp() * half.y);
        let z = layer_of(p).z();
        for (corner, uv) in [(-x - y, [0.0, 1.0]), (x - y, [1.0, 1.0]), (x + y, [1.0, 0.0]), (-x + y, [0.0, 0.0])] {
            q.positions.push((p.position + corner).extend(z).to_array());
            q.uvs.push(uv);
        }
        let light = map.light_at(p.position);
        let [r, g, b, a] = p.state.color;
        // XNA blended premultiplied colours; ours are straight.
        let straight = |c: f32, l: f32| if a > 0.0 { (c / a).min(1.0) * l } else { 0.0 };
        let color = [straight(r, light[0]), straight(g, light[1]), straight(b, light[2]), a];
        q.colors.extend([color; 4]);
    }

    let mut existing: HashMap<(&'static str, Layer), Handle<Mesh>> = HashMap::new();
    for (batch, mesh) in &mut batches {
        existing.insert((batch.texture, batch.layer), mesh.0.clone());
    }
    for (key, mesh) in &existing {
        if !quads.contains_key(key)
            && let Some(mut m) = meshes.get_mut(mesh)
        {
            *m = build(Quads::default());
        }
    }
    for (key, q) in quads {
        match existing.get(&key) {
            Some(handle) => {
                if let Some(mut m) = meshes.get_mut(handle) {
                    *m = build(q);
                }
            }
            None => {
                let Some(sprite) = map.sprites.get(key.0) else {
                    continue;
                };
                commands.spawn((
                    Name::new(format!("Particles {} {:?}", key.0, key.1)),
                    MapEntity,
                    Batch {
                        texture: key.0,
                        layer: key.1,
                    },
                    Mesh3d(meshes.add(build(q))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color_texture: Some(sprite.image.clone()),
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        double_sided: true,
                        cull_mode: None,
                        depth_bias: key.1.depth_bias(),
                        ..default()
                    })),
                    Transform::default(),
                    // Rebuilt every frame: never culled by stale bounds.
                    bevy::camera::visibility::NoFrustumCulling,
                ));
            }
        }
    }
}

fn build(q: Quads) -> Mesh {
    let n = q.positions.len() as u32 / 4;
    let indices = (0..n).flat_map(|i| [0, 1, 2, 0, 2, 3].map(|k| i * 4 + k)).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, q.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, q.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, q.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n as usize * 4])
        .with_inserted_indices(Indices::U32(indices))
}
