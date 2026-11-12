//! `cargo run --example lines`: coloured 3D ribbons viewed by an orbiting camera.
//! No game, map or Z-up assumption is needed by the line renderer.
//! Optional PNG capture: `cargo run --example lines -- /tmp/lines.png`.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tilerider::render::lines::{LineMaterial, LineMeshBuilder, LinePlugin};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, LinePlugin))
        .insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.06)))
        .add_systems(Startup, setup)
        .add_systems(Update, (orbit, capture))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LineMaterial>>,
) {
    for (to, color) in [
        (Vec3::X * 2.0, LinearRgba::RED),
        (Vec3::Y * 2.0, LinearRgba::GREEN),
        (Vec3::Z * 2.0, LinearRgba::BLUE),
        (
            Vec3::new(-1.5, 1.0, 1.5),
            LinearRgba::new(1.0, 0.5, 0.0, 0.5),
        ),
    ] {
        let mut lines = LineMeshBuilder::default();
        lines.add_line(Vec3::ZERO, to, 0.06);
        commands.spawn((
            Mesh3d(meshes.add(lines.build())),
            MeshMaterial3d(materials.add(LineMaterial { color, ..default() })),
            // The GPU expands each centre line beyond its CPU vertex bounds.
            NoFrustumCulling,
        ));
    }
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        DebandDither::Disabled,
        Transform::from_xyz(4.0, 3.0, 4.0).looking_at(Vec3::splat(0.5), Vec3::Y),
    ));
}

fn orbit(time: Res<Time>, mut camera: Query<&mut Transform, With<Camera3d>>) {
    let angle = time.elapsed_secs() * 0.25;
    for mut transform in &mut camera {
        *transform = Transform::from_xyz(angle.cos() * 5.0, 3.0, angle.sin() * 5.0)
            .looking_at(Vec3::splat(0.5), Vec3::Y);
    }
}

fn capture(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    *frame += 1;
    if *frame == 180 {
        if let Some(path) = std::env::args().nth(1) {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    } else if *frame == 200 && std::env::args().nth(1).is_some() {
        exit.write(AppExit::Success);
    }
}
