//! The camera: looks straight down from above, in perspective, and trails
//! its target (`Camera2D`, `Camera2DOperator`).
//!
//! It eases towards the target (`MoveSpeed` 4/s) but always looks at it,
//! so a fast car tilts the view a little in the direction it is going.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use crate::game::{GameState, LoadedMap};

/// Height above the ground in world units: about 25×15 units in view, as
/// the C# camera shows at its usual zoom (`MinHeight` 16 / `Scale` ≈ 0.9).
pub const CAMERA_HEIGHT: f32 = 18.0;
/// `Camera2D.MoveSpeed`.
const FOLLOW_RATE: f32 = 4.0;

/// What the camera follows.
#[derive(Component)]
pub struct CameraTarget;

#[derive(Component)]
pub struct MainCamera {
    /// Where the camera is over the ground (it eases towards the target).
    pub ground: Vec2,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(OnEnter(GameState::Playing), snap_to_target)
            .add_systems(
                PostUpdate,
                follow_target
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_4,
            near: 1.0,
            far: 1000.0,
            ..default()
        }),
        Tonemapping::None,
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        Msaa::Sample4,
        MainCamera { ground: Vec2::ZERO },
        Transform::from_xyz(0.0, 0.0, CAMERA_HEIGHT).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Start over the target (or the map's centre) instead of flying in.
fn snap_to_target(
    map: Res<LoadedMap>,
    targets: Query<&Transform, (With<CameraTarget>, Without<MainCamera>)>,
    mut cameras: Query<(&mut MainCamera, &mut Transform)>,
) {
    let target = targets.iter().next().map(|t| t.translation.truncate()).unwrap_or_else(|| {
        map.world(map.grid.width as f32 / 2.0, map.grid.height as f32 / 2.0)
    });
    for (mut cam, mut transform) in &mut cameras {
        cam.ground = target;
        *transform = Transform::from_translation(target.extend(CAMERA_HEIGHT))
            .looking_at(target.extend(0.0), Vec3::Y);
    }
}

fn follow_target(
    time: Res<Time>,
    targets: Query<&Transform, (With<CameraTarget>, Without<MainCamera>)>,
    mut cameras: Query<(&mut MainCamera, &mut Transform)>,
) {
    let Some(target) = targets.iter().next().map(|t| t.translation.truncate()) else {
        return;
    };
    let dt = time.delta_secs();
    for (mut cam, mut transform) in &mut cameras {
        let delta = target - cam.ground;
        cam.ground += delta * (FOLLOW_RATE * dt).min(1.0);
        *transform = Transform::from_translation(cam.ground.extend(CAMERA_HEIGHT))
            .looking_at(target.extend(0.0), Vec3::Y);
    }
}
