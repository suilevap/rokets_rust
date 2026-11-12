//! Perspective framing from XNA's `Camera2D` and `Camera2DOperatoAllInCadr`.
//!
//! A solo target leads by 0.7 seconds of velocity and zooms out with speed.
//! Multiple targets share the frame, including a six-radius driving margin.
//! Focus (1.9/s), camera position (4/s) and scale (0.9/s) ease separately;
//! scale stays between 1 and `CameraMinScaleBase` (0.4).
//!
//! Portrait screens turn the landscape view a quarter turn. The short side
//! keeps the same field of view; [`ViewRotation`] applies the same turn to
//! controls, and group framing accounts for that turn too.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use avian2d::prelude::LinearVelocity;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::game::{GameState, LoadedMap};

/// Closest camera height: XNA's `MinHeight` (512 / 32), at scale 1.
pub const CAMERA_HEIGHT: f32 = 16.0;
/// XNA's `Settings.CameraMinScaleBase`: furthest height is 16 / 0.4 = 40.
const MIN_SCALE: f32 = 0.4;
const FOCUS_RATE: f32 = 1.9;
const FOLLOW_RATE: f32 = 4.0;
const SCALE_RATE: f32 = 0.9;
const LOOK_AHEAD: f32 = 0.7;
const TARGET_MARGIN: f32 = 6.0;

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

/// A live entity included in camera framing. Attach to any focus entity;
/// velocity is optional, so fixed points can share the frame with cars.
#[derive(Component, Debug, Clone, Copy)]
pub struct CameraTarget {
    pub radius: f32,
    pub max_speed: f32,
}

#[derive(Component)]
pub struct MainCamera {
    /// Position above the ground, following the eased focus.
    pub ground: Vec2,
    /// Where the centre ray meets the ground (also used by HDR adaptation).
    pub focus: Vec2,
    /// Zoom: height is `CAMERA_HEIGHT / scale`.
    pub scale: f32,
}

impl Default for MainCamera {
    fn default() -> Self {
        Self {
            ground: Vec2::ZERO,
            focus: Vec2::ZERO,
            scale: 1.0,
        }
    }
}

impl MainCamera {
    fn transform(&self, up: Vec3) -> Transform {
        Transform::from_translation(self.ground.extend(CAMERA_HEIGHT / self.scale))
            .looking_at(self.focus.extend(0.0), up)
    }
}

#[derive(Clone, Copy)]
struct FocusTarget {
    position: Vec2,
    velocity: Vec2,
    radius: f32,
    max_speed: f32,
}

#[derive(Debug, PartialEq)]
struct Framing {
    focus: Vec2,
    scale: f32,
}

/// XNA's speed framing for one target, or midpoint/extent framing for a group.
fn framing(
    targets: impl IntoIterator<Item = FocusTarget>,
    view: &ViewRotation,
    projection: &PerspectiveProjection,
) -> Option<Framing> {
    let mut targets = targets.into_iter();
    let first = targets.next()?;
    let into_screen = Vec2::from_angle(-view.angle);
    let bounds = |target: FocusTarget| {
        let p = into_screen.rotate(target.position);
        let margin = Vec2::splat(target.radius.max(0.0) * TARGET_MARGIN);
        (p - margin, p + margin)
    };
    let (mut min, mut max) = bounds(first);
    let mut sum = first.position;
    let mut count = 1;
    for target in targets {
        let (lo, hi) = bounds(target);
        min = min.min(lo);
        max = max.max(hi);
        sum += target.position;
        count += 1;
    }
    if count == 1 {
        let speed = if first.max_speed > 0.0 {
            (first.velocity.length() / first.max_speed).clamp(0.0, 1.0)
        } else {
            0.0
        };
        return Some(Framing {
            focus: first.position + first.velocity * LOOK_AHEAD,
            scale: 1.0 - (1.0 - MIN_SCALE) * speed,
        });
    }
    let focus = sum / count as f32;
    let centre = into_screen.rotate(focus);
    let extent = (max - centre).max(centre - min);
    let tan_y = (projection.fov * 0.5).tan();
    let height = (extent.x / (tan_y * projection.aspect_ratio))
        .max(extent.y / tan_y)
        .max(CAMERA_HEIGHT);
    Some(Framing {
        focus,
        scale: (CAMERA_HEIGHT / height).clamp(MIN_SCALE, 1.0),
    })
}

/// Exponential easing preserves the rates across different frame times.
fn ease(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

type CameraTargets<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static CameraTarget,
        Option<&'static LinearVelocity>,
    ),
    Without<MainCamera>,
>;

fn target_snapshot(
    (transform, target, velocity): (&Transform, &CameraTarget, Option<&LinearVelocity>),
) -> FocusTarget {
    FocusTarget {
        position: transform.translation.truncate(),
        velocity: velocity.map_or(Vec2::ZERO, |v| v.0),
        radius: target.radius,
        max_speed: target.max_speed,
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewRotation>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PreUpdate, fit_view_to_window)
            .add_systems(
                OnEnter(GameState::Playing),
                snap_to_target.after(crate::car::spawn_player_car),
            )
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
        MainCamera::default(),
        Transform::from_xyz(0.0, 0.0, CAMERA_HEIGHT).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Start over the target (or the map's centre) instead of flying in.
fn snap_to_target(
    map: Res<LoadedMap>,
    view: Res<ViewRotation>,
    targets: CameraTargets,
    mut cameras: Query<(&mut MainCamera, &mut Transform, &Projection)>,
) {
    for (mut cam, mut transform, projection) in &mut cameras {
        let Projection::Perspective(projection) = projection else {
            continue;
        };
        let frame =
            framing(targets.iter().map(target_snapshot), &view, projection).unwrap_or_else(|| {
                Framing {
                    focus: map.world(map.grid.width as f32 / 2.0, map.grid.height as f32 / 2.0),
                    scale: 1.0,
                }
            });
        cam.ground = frame.focus;
        cam.focus = frame.focus;
        cam.scale = frame.scale;
        *transform = cam.transform(view.up());
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
    targets: CameraTargets,
    mut cameras: Query<(&mut MainCamera, &mut Transform, &Projection)>,
) {
    let dt = time.delta_secs();
    for (mut cam, mut transform, projection) in &mut cameras {
        let Projection::Perspective(projection) = projection else {
            continue;
        };
        let Some(frame) = framing(targets.iter().map(target_snapshot), &view, projection) else {
            continue;
        };
        cam.focus = cam.focus.lerp(frame.focus, ease(FOCUS_RATE, dt));
        cam.ground = cam.ground.lerp(cam.focus, ease(FOLLOW_RATE, dt));
        cam.scale += (frame.scale - cam.scale) * ease(SCALE_RATE, dt);
        *transform = cam.transform(view.up());
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
