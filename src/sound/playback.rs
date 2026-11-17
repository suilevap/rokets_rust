//! Plays the cues with Bevy's audio (`sound` feature): one-shots as
//! sounds that despawn when done, faded by distance from the camera's
//! focus ([`heard`]); the car's loops as looping players steered every
//! frame. Sounds missing from `assets/` are skipped with a warning.

use std::collections::HashMap;

use bevy::audio::{AudioSink, AudioSinkPlayback, PlaybackMode, Volume};
use bevy::prelude::*;

use super::{CarSounds, LoopCue, NAMES, PlaySound, heard};
use crate::game::MapEntity;
use crate::render::camera::MainCamera;

#[derive(Resource)]
struct Clips(HashMap<&'static str, Handle<AudioSource>>);

/// The players of a car's loops, made on first use.
#[derive(Component, Default)]
struct LoopPlayers {
    engine: Option<(&'static str, Entity)>,
    squeal: Option<(&'static str, Entity)>,
}

pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(
            Update,
            (play_once, play_loops)
                .after(super::log_cues)
                .run_if(resource_exists::<Clips>),
        );
    }
}

fn load(mut commands: Commands, server: Res<AssetServer>) {
    let clips = NAMES
        .iter()
        .filter_map(|&name| match crate::assets::resolve(&format!("Sounds/{name}"), "wav") {
            Some(path) => Some((name, server.load(path))),
            None => {
                warn!("sound {name:?} not in assets/");
                None
            }
        })
        .collect();
    commands.insert_resource(Clips(clips));
}

fn ear(cameras: &Query<&MainCamera>) -> Vec2 {
    cameras.iter().next().map_or(Vec2::ZERO, |c| c.focus)
}

fn play_once(mut commands: Commands, mut cues: MessageReader<PlaySound>, clips: Res<Clips>, cameras: Query<&MainCamera>) {
    let ear = ear(&cameras);
    for cue in cues.read() {
        let gain = heard(cue.volume, cue.at, ear);
        let Some(clip) = clips.0.get(cue.name).filter(|_| gain > 0.0) else {
            continue;
        };
        commands.spawn((
            Name::new(format!("Sound {}", cue.name)),
            AudioPlayer::new(clip.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(gain),
                speed: 2f32.powf(cue.pitch.clamp(-1.0, 1.0)),
                ..default()
            },
        ));
    }
}

fn play_loops(
    mut commands: Commands,
    clips: Res<Clips>,
    cameras: Query<&MainCamera>,
    mut cars: Query<(Entity, &CarSounds, &Transform, Option<&mut LoopPlayers>)>,
    mut sinks: Query<&mut AudioSink>,
) {
    let ear = ear(&cameras);
    for (car, sounds, t, players) in &mut cars {
        let Some(mut players) = players else {
            commands.entity(car).insert(LoopPlayers::default());
            continue;
        };
        let at = Some(t.translation.truncate());
        let players = &mut *players;
        for (slot, cue) in [(&mut players.engine, sounds.engine), (&mut players.squeal, sounds.squeal)] {
            steer(&mut commands, &clips, &mut sinks, slot, cue, |v| heard(v, at, ear));
        }
    }
}

/// Keeps one looping player in step with its cue: made on first use,
/// remade when the cue names another sound, paused when there is none.
fn steer(
    commands: &mut Commands,
    clips: &Clips,
    sinks: &mut Query<&mut AudioSink>,
    slot: &mut Option<(&'static str, Entity)>,
    cue: Option<LoopCue>,
    gain: impl Fn(f32) -> f32,
) {
    if let (Some((name, entity)), Some(cue)) = (*slot, cue)
        && name != cue.name
    {
        commands.entity(entity).despawn();
        *slot = None;
    }
    match (*slot, cue) {
        (None, Some(cue)) => {
            if let Some(clip) = clips.0.get(cue.name) {
                let e = commands
                    .spawn((Name::new("Loop sound"), MapEntity, AudioPlayer::new(clip.clone()), PlaybackSettings::LOOP))
                    .id();
                *slot = Some((cue.name, e));
            }
        }
        (Some((_, entity)), cue) => {
            let Ok(mut sink) = sinks.get_mut(entity) else {
                return;
            };
            match cue {
                Some(cue) => {
                    sink.set_speed(2f32.powf(cue.pitch.clamp(-1.0, 1.0)));
                    sink.set_volume(Volume::Linear(gain(cue.volume)));
                    if sink.is_paused() {
                        sink.play();
                    }
                }
                None => sink.pause(),
            }
        }
        (None, None) => {}
    }
}
