//! `tilerider [-m MAP] [--screenshot PATH [--frames N]]`
//!
//! `-m` picks the map (a name from `assets/Maps/Singleplayer`, default
//! `EditorTest`). `--screenshot` saves the frame after N frames of play
//! (default 90) and exits, for comparing with the C# game.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;

use tilerider::TileRiderPlugin;
use tilerider::game::GameState;

#[derive(Resource, Clone, Debug)]
struct ScreenshotRequest {
    path: String,
    frames: u32,
    counted: u32,
    taken: bool,
}

fn parse_args() -> (String, Option<ScreenshotRequest>) {
    let mut map = "EditorTest".to_string();
    let mut shot: Option<ScreenshotRequest> = None;
    let mut frames = 90;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-m" | "--map" => map = args.next().unwrap_or(map),
            "--screenshot" => {
                shot = args.next().map(|path| ScreenshotRequest {
                    path,
                    frames: 0,
                    counted: 0,
                    taken: false,
                })
            }
            "--frames" => frames = args.next().and_then(|v| v.parse().ok()).unwrap_or(frames),
            _ => {}
        }
    }
    if let Some(s) = shot.as_mut() {
        s.frames = frames;
    }
    (map, shot)
}

fn main() {
    let (map, shot) = parse_args();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "TileRider".into(),
            resolution: WindowResolution::new(1280, 720),
            canvas: Some("#bevy".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }))
    .add_plugins(TileRiderPlugin { map });
    if let Some(shot) = shot {
        app.insert_resource(shot)
            .add_systems(Update, take_screenshot.run_if(in_state(GameState::Playing)));
    }
    app.run();
}

fn take_screenshot(mut commands: Commands, mut shot: ResMut<ScreenshotRequest>, mut exit: MessageWriter<AppExit>) {
    shot.counted += 1;
    if !shot.taken && shot.counted >= shot.frames {
        shot.taken = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(shot.path.clone()));
    } else if shot.taken && shot.counted >= shot.frames + 10 {
        exit.write(AppExit::Success);
    }
}
