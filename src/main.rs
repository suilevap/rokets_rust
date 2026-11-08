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
//! `--size WxH` sets the window size (default 1280x720; e.g. 720x1280 to
//! try portrait).
//!
//! In the browser the page's URL parameters are the options, without `--`:
//! `?map=Middleware&start=84,9.5&drive=1,0:3`. Only `map`, `start` and
//! `drive` apply there; the file-writing options are native only.

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
    size: (u32, u32),
}

/// The command line, or the page's URL parameters in the browser.
fn raw_args() -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args().skip(1).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        web::url_args()
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "export function search() { return window.location.search; }")]
    extern "C" {
        fn search() -> String;
    }

    /// Options the browser build accepts, as URL parameters.
    const WEB_OPTIONS: &[&str] = &["map", "start", "drive"];

    /// `?map=Middleware&start=84,9.5` → `["--map", "Middleware", "--start", "84,9.5"]`.
    pub fn url_args() -> Vec<String> {
        let query = search();
        let mut args = Vec::new();
        for pair in query
            .trim_start_matches('?')
            .split('&')
            .filter(|p| !p.is_empty())
        {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = decode(value);
            if WEB_OPTIONS.contains(&name) && !value.is_empty() {
                args.push(format!("--{name}"));
                args.push(value);
            }
        }
        args
    }

    /// Percent-decoding for the few characters options use (`,` `;` `:`).
    fn decode(s: &str) -> String {
        let bytes = s.replace('+', " ").into_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%'
                && i + 2 < bytes.len()
                && let Ok(b) =
                    u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(b);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}

fn parse_args() -> Args {
    let mut map = "EditorTest".to_string();
    let mut drive = None;
    let mut record_dir = None;
    let mut start = None;
    let mut size = (1280, 720);
    let mut shot: Option<ScreenshotRequest> = None;
    let mut frames = 90;
    let mut args = raw_args().into_iter();
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
            "--size" => {
                if let Some((w, h)) = args
                    .next()
                    .as_deref()
                    .and_then(|v| v.split_once('x'))
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                {
                    size = (w, h);
                }
            }
            "--frames" => frames = args.next().and_then(|v| v.parse().ok()).unwrap_or(frames),
            _ => {}
        }
    }
    if let Some(s) = shot.as_mut() {
        s.frames = frames;
    }
    let record = record_dir.map(|dir| RecordRequest {
        dir,
        frames,
        counted: 0,
    });
    Args {
        map,
        shot,
        drive,
        record,
        start,
        size,
    }
}

fn main() {
    let Args {
        map,
        shot,
        drive,
        record,
        start,
        size,
    } = parse_args();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            // No `.meta` files ship with the assets; asking for them is a
            // failed request per asset in the browser.
            .set(AssetPlugin {
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "TileRider".into(),
                    resolution: WindowResolution::new(size.0, size.1),
                    canvas: Some("#bevy".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    ..default()
                }),
                ..default()
            }),
    )
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

fn take_screenshot(
    mut commands: Commands,
    mut shot: ResMut<ScreenshotRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    shot.counted += 1;
    if !shot.taken && shot.counted >= shot.frames {
        shot.taken = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(shot.path.clone()));
    } else if shot.taken && shot.counted >= shot.frames + 10 {
        exit.write(AppExit::Success);
    }
}

fn record_frame(
    mut commands: Commands,
    mut rec: ResMut<RecordRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if rec.counted < rec.frames {
        let path = format!("{}/frame_{:05}.png", rec.dir, rec.counted);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    } else if rec.counted > rec.frames + 10 {
        exit.write(AppExit::Success);
    }
    rec.counted += 1;
}
