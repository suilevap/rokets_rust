//! The darkening of the floor at the foot of walls ("ambient occlusion").
//!
//! C# origin: `LightManager.CreateWallAmbientOcclusion` (2013, "Added
//! Occlusion Light emulation… distance field"). One texel per map cell
//! from `MapManager.DistanceField`: black at half strength on walls, at a
//! quarter on floor cells closer than two cells to one, clear elsewhere
//! and on the map's outer cells. Stretched over the map with linear
//! filtering, it fades over about a cell and a half from each wall's foot.
//! Drawn on the ground, so walls hide it and floor tiles and the gaps
//! between them darken. Maps with cells of 96 pixels or more (3 world
//! units) have none: their cells are big enough without.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game::{GameState, LoadedMap, MapEntity};
use crate::map::grid::MapGrid;
use crate::render::layers::{AO_DEPTH_BIAS, AO_Z};

/// Cells this big (world units) or bigger get no darkening.
const LARGEST_CELL: f32 = 3.0;

pub struct WallAoPlugin;

impl Plugin for WallAoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_wall_ao);
    }
}

/// How dark each cell is, 0..1, row-major: half on walls, a quarter on
/// floor within two cells of a wall, none on the outer cells.
pub fn wall_ao(grid: &MapGrid) -> Vec<f32> {
    let (w, h) = (grid.width, grid.height);
    let distance = grid.chamfer_distance();
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            // `x > 1` as C# had it: the first two columns and rows stay clear.
            let inside = x > 1 && x + 1 < w && y > 1 && y + 1 < h;
            let d = distance[y * w + x];
            if !inside {
                0.0
            } else if d < 1.0 {
                0.5
            } else if d < 2.0 {
                0.25
            } else {
                0.0
            }
        })
        .collect()
}

fn spawn_wall_ao(
    mut commands: Commands,
    map: Res<LoadedMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let cell = map.cell.0;
    if cell >= LARGEST_CELL {
        return;
    }
    let (w, h) = (map.grid.width, map.grid.height);
    let data = wall_ao(&map.grid)
        .into_iter()
        .flat_map(|a| [0, 0, 0, (a * 255.0).round() as u8])
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let size = Vec2::new(w as f32, h as f32) * cell;
    let center = map.world(w as f32 / 2.0, h as f32 / 2.0);
    commands.spawn((
        Name::new("Wall AO"),
        MapEntity,
        Mesh3d(meshes.add(Rectangle::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(images.add(image)),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            depth_bias: AO_DEPTH_BIAS,
            ..default()
        })),
        Transform::from_translation(center.extend(AO_Z)),
    ));
}
