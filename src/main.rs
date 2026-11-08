//! `tilerider [-m MAP] [--screenshot PATH [--frames N]] [--drive SCRIPT]`
//!
//! `-m` picks the map (a name from `assets/Maps/Singleplayer`, default
//! `EditorTest`). `--screenshot` saves the frame after N frames of play
//! (default 90) and exits, for comparing with the C# game. `--drive`
//! drives the car by a script and logs its path (see `script.rs`).
//! `--record DIR` saves every frame of play as `DIR/frame_NNNNN.png`, with
//! time stepped exactly 1/30 s per frame (whatever the rendering speed),
//! for `--frames N` frames, then exits: turn them into a video with ffmpeg.
//! `--start x,y` puts the car at that map cell instead of the map's start.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;

use tilerider::TileRiderPlugin;
use tilerider::game::GameState;
use tilerider::script::DriveScript;

#[derive(Resource, Clone, Debug)]
struct RecordRequest {
    dir: String,
    frames: u32,
    counted: u32,
}

#[derive(Resource, Clone, Debug)]
struct ScreenshotRequest {
    path: String,
    frames: u32,
    counted: u32,
    taken: bool,
}

struct Args {
    map: String,
    shot: Option<ScreenshotRequest>,
    drive: Option<DriveScript>,
    record: Option<RecordRequest>,
    start: Option<Vec2>,
}

fn parse_args() -> Args {
    let mut map = "EditorTest".to_string();
    let mut drive = None;
    let mut record_dir = None;
    let mut start = None;
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
            "--drive" => {
                drive = args.next().and_then(|s| {
                    let parsed = DriveScript::parse(&s);
                    if parsed.is_none() {
                        eprintln!("bad --drive script {s:?}; expected x,y:seconds;...");
                    }
                    parsed
                })
            }
            "--record" => record_dir = args.next(),
            "--start" => {
                start = args.next().and_then(|v| {
                    let (x, y) = v.split_once(',')?;
                    Some(Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
                })
            }
            "--frames" => frames = args.next().and_then(|v| v.parse().ok()).unwrap_or(frames),
            _ => {}
        }
    }
    if let Some(s) = shot.as_mut() {
        s.frames = frames;
    }
    let record = record_dir.map(|dir| RecordRequest { dir, frames, counted: 0 });
    Args {
        map,
        shot,
        drive,
        record,
        start,
    }
}

fn main() {
    let Args {
        map,
        shot,
        drive,
        record,
        start,
    } = parse_args();
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
    if let Some(start) = start {
        app.insert_resource(tilerider::car::StartOverride(start));
    }
    if let Some(drive) = drive {
        app.insert_resource(drive);
    }
    if let Some(record) = record {
        std::fs::create_dir_all(&record.dir).expect("create the --record folder");
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 30.0),
        ))
        .insert_resource(record)
        .add_systems(Update, record_frame.run_if(in_state(GameState::Playing)));
    }
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

fn record_frame(mut commands: Commands, mut rec: ResMut<RecordRequest>, mut exit: MessageWriter<AppExit>) {
    if rec.counted < rec.frames {
        let path = format!("{}/frame_{:05}.png", rec.dir, rec.counted);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    } else if rec.counted > rec.frames + 10 {
        exit.write(AppExit::Success);
    }
    rec.counted += 1;
}
