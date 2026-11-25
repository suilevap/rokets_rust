//! Plays the cues with Bevy's audio (`sound` feature): one-shots as
//! sounds that despawn when done, faded by distance from the camera's
//! focus ([`heard`]); the car's loops as looping players steered every
//! frame. Sounds missing from `assets/` are skipped with a warning.
//! The level music is one more player ([`MusicPlaybackPlugin`]).

use std::collections::HashMap;

use bevy::audio::{AudioSink, AudioSinkPlayback, PlaybackMode, Volume};
use bevy::prelude::*;

use super::music::{Jukebox, MusicCue, send_cues};
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
    gun: Option<(&'static str, Entity)>,
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
        .map(|&name| (name, server.load(crate::assets::sound_path(name))))
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
        for (slot, cue) in [
            (&mut players.engine, sounds.engine),
            (&mut players.squeal, sounds.squeal),
            (&mut players.gun, sounds.gun),
        ] {
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

/// The level's songs, loaded when a map names them (the web build fetches
/// only the songs the map in play uses), and the player of the one playing.
#[derive(Resource, Default)]
struct Songs {
    /// By the song's name.
    clips: HashMap<String, Handle<AudioSource>>,
    player: Option<Entity>,
}

/// Plays the [`Jukebox`]'s songs (`MediaPlayer`), at its volume.
pub struct MusicPlaybackPlugin;

impl Plugin for MusicPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Songs>()
            .add_systems(Update, (load_songs, play_music).chain().after(send_cues));
    }
}

fn load_songs(jukebox: Res<Jukebox>, server: Res<AssetServer>, mut songs: ResMut<Songs>) {
    for name in jukebox.level_tracks() {
        if songs.clips.contains_key(name) {
            continue;
        }
        let clip = server.load(crate::assets::music_path(name));
        songs.clips.insert(name.to_string(), clip);
    }
}

fn play_music(
    mut commands: Commands,
    mut cues: MessageReader<MusicCue>,
    mut songs: ResMut<Songs>,
    mut jukebox: ResMut<Jukebox>,
    mut sinks: Query<&mut AudioSink>,
) {
    for cue in cues.read() {
        if let Some(old) = songs.player.take() {
            commands.entity(old).despawn();
        }
        let MusicCue::Play { track, looped } = cue else {
            continue;
        };
        let Some(clip) = songs.clips.get(track) else {
            // A song that cannot play is a song that has ended.
            jukebox.song_ended();
            continue;
        };
        let player = commands
            .spawn((
                Name::new(format!("Music {track}")),
                AudioPlayer::new(clip.clone()),
                PlaybackSettings {
                    mode: if *looped { PlaybackMode::Loop } else { PlaybackMode::Once },
                    volume: Volume::Linear(jukebox.volume()),
                    ..default()
                },
            ))
            .id();
        songs.player = Some(player);
    }
    let Some(player) = songs.player else {
        return;
    };
    // The sink is there once the song has loaded and started.
    let Ok(mut sink) = sinks.get_mut(player) else {
        return;
    };
    sink.set_volume(Volume::Linear(jukebox.volume()));
    if sink.empty() {
        commands.entity(player).despawn();
        songs.player = None;
        jukebox.song_ended();
    }
}
