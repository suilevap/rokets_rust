//! Debug text until the real HUD: map name, coins, controls, level end,
//! frames per second, the additive light's strength. `R` restarts the map;
//! `N` skips to the next one.
//! The frame rate is also logged every few seconds, to compare builds.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::controls::ActiveScheme;
use crate::game::{ChangeMap, CurrentMapName, GameState};
use crate::pickups::CoinCount;
use crate::triggers::LevelEnd;

#[derive(Component)]
struct DebugText;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_systems(Startup, spawn_text)
            .add_systems(Update, (update_text, map_keys).run_if(in_state(GameState::Playing)))
            .add_systems(Update, log_frame_rate);
    }
}

fn spawn_text(mut commands: Commands) {
    commands.spawn((
        DebugText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));
}

/// Smoothed frames per second and milliseconds per frame.
fn frame_rate(diagnostics: &DiagnosticsStore) -> Option<(f64, f64)> {
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS)?.smoothed()?;
    let ms = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)?.smoothed()?;
    Some((fps, ms))
}

/// Every five seconds of real time, the frame rate to the log.
fn log_frame_rate(time: Res<Time<Real>>, diagnostics: Res<DiagnosticsStore>, mut last: Local<f32>) {
    let now = time.elapsed_secs();
    if now - *last < 5.0 {
        return;
    }
    *last = now;
    if let Some((fps, ms)) = frame_rate(&diagnostics) {
        info!("{fps:.1} fps, {ms:.1} ms a frame");
    }
}

fn update_text(
    diagnostics: Res<DiagnosticsStore>,
    boost: Res<crate::render::additive_light::GlowBoost>,
    map: Option<Res<crate::game::LoadedMap>>,
    name: Res<CurrentMapName>,
    coins: Res<CoinCount>,
    scheme: Res<ActiveScheme>,
    end: Res<LevelEnd>,
    mut text: Query<&mut Text, With<DebugText>>,
) {
    let done = if end.timer.is_some() { "\nLEVEL COMPLETE" } else { "" };
    // The additive light's strength, on maps that have one.
    let glow = map
        .filter(|m| m.light.additive.is_some())
        .map_or(String::new(), |_| format!("  glow {:.2}", boost.0));
    let fps = frame_rate(&diagnostics).map_or(String::new(), |(fps, ms)| format!("  {fps:.0} fps {ms:.1} ms"));
    let s = format!(
        "{}  gears {}/{}  [{:?}]  R restart, N next{fps}{glow}{}",
        name.0, coins.collected, coins.total, *scheme, done
    );
    for mut t in &mut text {
        if t.0 != s {
            t.0 = s.clone();
        }
    }
}

fn map_keys(keys: Res<ButtonInput<KeyCode>>, mut change: MessageWriter<ChangeMap>) {
    if keys.just_pressed(KeyCode::KeyR) {
        change.write(ChangeMap::Restart);
    }
    if keys.just_pressed(KeyCode::KeyN) {
        change.write(ChangeMap::Next);
    }
}
