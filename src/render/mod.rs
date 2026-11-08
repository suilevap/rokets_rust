//! Rendering: the map meshes, sprites as lit quads, the camera.
//!
//! The world is real 3D geometry in the physics plane (x, y) with z up:
//! walls rise above z = 0, floor tiles sit below it, cars and pickups ride
//! just above it. Everything is unlit (colours carry the baked light), so
//! the picture does not depend on Bevy's lighting.

pub mod camera;
pub mod layers;
pub mod map_mesh;
pub mod sprite;

use bevy::prelude::*;

use crate::game::{GameState, LoadedMap, MapEntity};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        // Beyond the lit background plane the world is dark.
        app.insert_resource(ClearColor(Color::BLACK))
            .add_plugins((camera::CameraPlugin, sprite::SpritePlugin))
            .add_systems(OnEnter(GameState::Playing), spawn_map_meshes);
    }
}

fn spawn_map_meshes(
    mut commands: Commands,
    map: Res<LoadedMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (floor, walls) = map_mesh::build_map_meshes(&map);
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("Floor"),
        MapEntity,
        Mesh3d(meshes.add(floor)),
        MeshMaterial3d(material.clone()),
        Transform::default(),
    ));
    commands.spawn((
        Name::new("Walls mesh"),
        MapEntity,
        Mesh3d(meshes.add(walls)),
        MeshMaterial3d(material),
        Transform::default(),
    ));
}
