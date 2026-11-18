//! `CameraShowPath`: the camera flies along a path (`Camera2DOperatorFollowPath`),
//! an intro over the level.
//!
//! It moves along at `Speed` (world units a second) and, once there,
//! runs `ActionOnEnd`; with `IsReverseAtTheEnd` it waits `WaitInSec` and
//! flies back, then runs `ActionOnReverseEnd`. With `PauseWhileMove` the
//! physics stand still while it moves. It looks from `ScaleAmount` out
//! (0 close, 1 as far as the camera goes). It stays at the end until an
//! action puts the camera back on its targets (`FocusOnEntity` and the
//! like: C#'s `GetDefaultOperator`).

use avian2d::prelude::*;
use bevy::prelude::*;

use super::clone::point_at;
use super::{AddMapAction, Run, RunAction};
use crate::content::actions::defs::CameraShowPath;
use crate::game::{GameState, LoadedMap};
use crate::render::camera::{CameraOverride, MIN_SCALE, MainCamera};

/// How near the camera must be to the path's end to have arrived.
const ARRIVED: f32 = 0.5;

/// The camera's flight, while it lasts.
#[derive(Resource, Debug)]
pub struct CameraPath {
    points: Vec<Vec2>,
    length: f32,
    speed: f32,
    at: f32,
    /// 1 out, -1 back, 0 still.
    direction: f32,
    /// Seconds before it flies back, when waiting at the end.
    wait_left: Option<f32>,
    wait: f32,
    reverse: bool,
    on_end: Option<String>,
    on_reverse_end: Option<String>,
    actor: Option<i64>,
    pause: bool,
    scale: f32,
}

pub struct CameraPathPlugin;

impl Plugin for CameraPathPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<CameraShowPath, _>(show_path)
            .add_systems(OnEnter(GameState::Playing), stop_camera_path)
            .add_systems(Update, fly.run_if(resource_exists::<CameraPath>).run_if(in_state(GameState::Playing)));
    }
}

/// Back to the camera's targets; the physics run again.
pub fn stop_camera_path(
    mut commands: Commands,
    over: Option<ResMut<CameraOverride>>,
    physics: Option<ResMut<Time<Physics>>>,
) {
    commands.remove_resource::<CameraPath>();
    if let Some(mut over) = over {
        over.0 = None;
    }
    if let Some(mut physics) = physics {
        physics.unpause();
    }
}

fn show_path(
    In(run): In<Run<CameraShowPath>>,
    map: Res<LoadedMap>,
    mut physics: ResMut<Time<Physics>>,
    mut commands: Commands,
) -> bool {
    let d = run.data;
    let points: Vec<Vec2> = d.path.iter().map(|p| map.world(p.x, p.y)).collect();
    if points.is_empty() {
        return false;
    }
    let length = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    if d.pause_while_move {
        physics.pause();
    }
    commands.insert_resource(CameraPath {
        points,
        length,
        speed: d.speed,
        at: 0.0,
        direction: 1.0,
        wait_left: None,
        wait: d.wait_in_sec,
        reverse: d.is_reverse_at_the_end,
        on_end: d.action_on_end.filter(|a| !a.trim().is_empty()),
        on_reverse_end: d.action_on_reverse_end.filter(|a| !a.trim().is_empty()),
        actor: run.actor,
        pause: d.pause_while_move,
        // `SetScaleAmount`.
        scale: MIN_SCALE + (1.0 - MIN_SCALE) * (1.0 - d.scale_amount),
    });
    true
}

fn fly(
    time: Res<Time>,
    mut path: ResMut<CameraPath>,
    mut over: ResMut<CameraOverride>,
    mut physics: ResMut<Time<Physics>>,
    cameras: Query<&MainCamera>,
    mut run: MessageWriter<RunAction>,
) {
    let dt = time.delta_secs();
    if let Some(left) = path.wait_left.as_mut() {
        *left -= dt;
        if *left <= 0.0 {
            path.wait_left = None;
            path.direction = -1.0;
            if path.pause {
                physics.pause();
            }
        }
    }
    path.at = (path.at + path.speed * path.direction * dt).clamp(0.0, path.length);
    let point = point_at(&path.points, path.at);
    over.0 = Some((point, path.scale));
    let arrived = cameras.iter().any(|c| c.focus.distance(point) < ARRIVED);
    if !arrived {
        return;
    }
    let actor = path.actor;
    if path.direction > 0.0 && path.at >= path.length {
        path.direction = 0.0;
        if path.pause {
            physics.unpause();
        }
        if path.reverse {
            path.wait_left = Some(path.wait);
        }
        if let Some(name) = path.on_end.clone() {
            run.write(RunAction { name, actor });
        }
    } else if path.direction < 0.0 && path.at <= 0.0 {
        path.direction = 0.0;
        if path.pause {
            physics.unpause();
        }
        if let Some(name) = path.on_reverse_end.clone() {
            run.write(RunAction { name, actor });
        }
    }
}
