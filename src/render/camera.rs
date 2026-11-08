//! The camera: looks straight down from above, in perspective, and trails
//! its target (`Camera2D`, `Camera2DOperator`).
//!
//! It eases towards the target (`MoveSpeed` 4/s) but always looks at it,
//! so a fast car tilts the view a little in the direction it is going.
//!
//! **Portrait screens** (phones held upright; the C# game was landscape
//! only) see the same picture as landscape, turned a quarter turn: the
//! screen's long side still runs along the map's x axis, so up on the screen
//! is right on the map. The screen's short side always shows as much of the
//! world as the C# camera's height did (about 15 units), the long side more.
//! [`ViewRotation`] turns screen directions into the world for the controls.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::game::{GameState, LoadedMap};

/// Height above the ground in world units: about 25×15 units in view, as
/// the C# camera shows at its usual zoom (`MinHeight` 16 / `Scale` ≈ 0.9).
pub const CAMERA_HEIGHT: f32 = 18.0;
/// `Camera2D.MoveSpeed`.
const FOLLOW_RATE: f32 = 4.0;

/// How fast the view turns between landscape and portrait, per second.
const TURN_RATE: f32 = 6.0;
/// Width / height beyond which the view switches; between them it stays as
/// it is, so a nearly square window does not flip back and forth.
const PORTRAIT_BELOW: f32 = 0.95;
const LANDSCAPE_ABOVE: f32 = 1.05;

/// The vertical field of view that shows the landscape amount of world on
/// the screen's short side.
pub fn fov_for_aspect(aspect: f32) -> f32 {
    if aspect >= 1.0 {
        FRAC_PI_4
    } else {
        2.0 * ((FRAC_PI_4 / 2.0).tan() / aspect).atan()
    }
}

/// How screen directions turn into world directions: by `angle` radians
/// (0 in landscape, a quarter turn clockwise in portrait).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct ViewRotation {
    /// The current angle, easing towards `target`.
    pub angle: f32,
    target: f32,
}

impl ViewRotation {
    /// Turns a direction on the screen (x right, y up) into the world.
    pub fn screen_to_world(&self, v: Vec2) -> Vec2 {
        Vec2::from_angle(self.angle).rotate(v)
    }

    /// Picks landscape or portrait for a window's width / height.
    fn update_target(&mut self, aspect: f32) {
        if aspect < PORTRAIT_BELOW {
            self.target = -FRAC_PI_2;
        } else if aspect > LANDSCAPE_ABOVE {
            self.target = 0.0;
        }
    }

    /// The camera's up vector in the world.
    fn up(&self) -> Vec3 {
        self.screen_to_world(Vec2::Y).extend(0.0)
    }
}

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
        app.init_resource::<ViewRotation>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PreUpdate, fit_view_to_window)
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
    view: Res<ViewRotation>,
    targets: Query<&Transform, (With<CameraTarget>, Without<MainCamera>)>,
    mut cameras: Query<(&mut MainCamera, &mut Transform)>,
) {
    let target = targets
        .iter()
        .next()
        .map(|t| t.translation.truncate())
        .unwrap_or_else(|| map.world(map.grid.width as f32 / 2.0, map.grid.height as f32 / 2.0));
    for (mut cam, mut transform) in &mut cameras {
        cam.ground = target;
        *transform = Transform::from_translation(target.extend(CAMERA_HEIGHT))
            .looking_at(target.extend(0.0), view.up());
    }
}

/// Sets the field of view for the window's shape and eases the view's turn
/// between landscape and portrait.
fn fit_view_to_window(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut view: ResMut<ViewRotation>,
    mut cameras: Query<&mut Projection, With<MainCamera>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let (w, h) = (window.width(), window.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let aspect = w / h;
    let first = *view == ViewRotation::default() && aspect < PORTRAIT_BELOW;
    view.update_target(aspect);
    if first {
        view.angle = view.target;
    } else {
        let k = (TURN_RATE * time.delta_secs()).min(1.0);
        let next = view.angle + (view.target - view.angle) * k;
        if (next - view.angle).abs() > f32::EPSILON {
            view.angle = next;
        }
    }
    let fov = fov_for_aspect(aspect);
    for mut projection in &mut cameras {
        if let Projection::Perspective(p) = projection.as_mut()
            && (p.fov - fov).abs() > 1e-4
        {
            p.fov = fov;
        }
    }
}

fn follow_target(
    time: Res<Time>,
    view: Res<ViewRotation>,
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
            .looking_at(target.extend(0.0), view.up());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_side_keeps_the_landscape_view() {
        // Landscape: the vertical (short) side spans π/4.
        assert_eq!(fov_for_aspect(16.0 / 9.0), FRAC_PI_4);
        // Portrait: the horizontal (short) side spans π/4, so vertically more.
        let aspect = 9.0 / 16.0;
        let fov = fov_for_aspect(aspect);
        let horizontal = 2.0 * ((fov / 2.0).tan() * aspect).atan();
        assert!((horizontal - FRAC_PI_4).abs() < 1e-5);
        assert!(fov > FRAC_PI_4);
    }

    #[test]
    fn portrait_turns_screen_up_into_map_right() {
        let mut view = ViewRotation::default();
        view.update_target(9.0 / 16.0);
        view.angle = view.target;
        let up = view.screen_to_world(Vec2::Y);
        assert!((up - Vec2::X).length() < 1e-5, "{up}");
        let right = view.screen_to_world(Vec2::X);
        assert!((right - Vec2::NEG_Y).length() < 1e-5, "{right}");
        // Nearly square: no switch back.
        view.update_target(1.0);
        assert_eq!(view.target, -FRAC_PI_2);
        view.update_target(16.0 / 9.0);
        assert_eq!(view.target, 0.0);
    }
}
