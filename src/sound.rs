//! Sound effects (`SoundManager`, `SoundView`, `EffectManager`): one-shot
//! sounds on game events, and the car's engine and tyres.
//!
//! - **One-shots** ([`PlaySound`]): a sound at a point, heard from the
//!   camera's focus. C# placed it in 3D (`Apply3D`) and did not play what
//!   was out of earshot (`CanHearRadius`, 32 units, times the volume);
//!   here it fades linearly to silence at that distance, without panning.
//!   Pitch is in octaves, -1..1, as XNA's.
//! - **Hits** (`EffectManager.CollideEffect`): one of three bumps, louder
//!   the harder the blow, at a random pitch within half an octave.
//! - **Blasts**: `RocketDestroy` (seven times in ten) or `Explosion`, at
//!   the next pitch of a fixed round (`PlayOnceWithNextPith`).
//! - **Damage** to the player: one of three crunches; `Accident` when it
//!   is destroyed. **Gears**: `CoinCollected`. **Finish**: `EndLevel`.
//! - **Engine** (`Car.EngineSoundControl`): while the stick is held, the
//!   car's `EngineSound` loops, louder with the stick, higher with the
//!   speed (up to an octave at top speed); paused otherwise.
//! - **Tyres** (`Car.CreateWheelsTrailAndSound`): while the wheels slip
//!   the squeal eases (3/s) towards `0.4 + 0.6 × speed²` (`speed²`×0.9
//!   when turning on the spot), and plays while above 0.15; one of two
//!   squeals, picked anew each time it stops.

use std::collections::HashMap;

use avian2d::prelude::LinearVelocity;
use bevy::audio::{AudioSink, AudioSinkPlayback, PlaybackMode, Volume};
use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;
use std::f32::consts::FRAC_PI_4;

use crate::car::Car;
use crate::car::impact::CarImpact;
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::explosion::{Explosion, ExplosionSystems, Health, Hit};
use crate::game::{ChangeMap, GameState, LoadedMap, MapEntity};
use crate::pickups::CoinCollected;
use crate::render::camera::MainCamera;

/// `SoundManager.CanHearRadius`: 1024 px.
const HEARING: f32 = 32.0;

/// The sounds loaded at start, by name.
const NAMES: [&str; 16] = [
    "Engine",
    "EngineHover",
    "EngineTruck",
    "wheels",
    "Wheels1",
    "CarBom0",
    "CarBom1",
    "CarBom2",
    "CarDamaged0",
    "CarDamaged1",
    "CarDamaged2",
    "Accident",
    "RocketDestroy",
    "Explosion",
    "CoinCollected",
    "EndLevel",
];

/// `SoundManager._pitchArray`: the round of pitches for repeated blasts.
const PITCH_ROUND: [f32; 9] = [0.0, 0.5, -0.25, 0.25, 0.75, -0.5, 0.5, 1.0, -1.0];

#[derive(Resource)]
pub struct Sounds {
    clips: HashMap<&'static str, Handle<AudioSource>>,
    next_pitch: usize,
    rng: SmallRng,
}

impl Sounds {
    fn clip(&self, name: &str) -> Option<Handle<AudioSource>> {
        self.clips.get(name).cloned()
    }
}

/// Play a sound once. `at`: where it happens (none: at the listener).
#[derive(Message, Debug, Clone, Copy)]
pub struct PlaySound {
    pub name: &'static str,
    pub at: Option<Vec2>,
    /// Octaves, -1..1.
    pub pitch: f32,
    /// 0..1.
    pub volume: f32,
}

/// The car's looping engine and tyre sounds.
#[derive(Component, Debug)]
pub struct CarSounds {
    /// The car's `EngineSound`, if it has one.
    pub engine: Option<&'static str>,
    /// Octaves added to the tyres (`WheelsSoundPitch`).
    pub wheels_pitch: f32,
    squeal: f32,
    squeal_clip: usize,
    engine_player: Option<Entity>,
    wheel_player: Option<Entity>,
}

impl CarSounds {
    pub fn new(engine: Option<&str>, wheels_pitch: f32) -> CarSounds {
        CarSounds {
            engine: engine.and_then(|e| NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(e))),
            wheels_pitch,
            squeal: 0.0,
            squeal_clip: 0,
            engine_player: None,
            wheel_player: None,
        }
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>()
            .add_systems(Startup, load_sounds)
            .add_systems(
                Update,
                (
                    (impact_sounds, blast_sounds, damage_sounds, coin_sounds, finish_sounds),
                    play_sounds,
                    car_loops,
                )
                    .chain()
                    .after(ExplosionSystems)
                    .run_if(resource_exists::<Sounds>),
            );
    }
}

fn load_sounds(mut commands: Commands, server: Res<AssetServer>) {
    let clips = NAMES
        .iter()
        .filter_map(|&name| {
            let path = crate::assets::resolve(&format!("Sounds/{name}"), "wav");
            if path.is_none() {
                warn!("sound {name:?} not found");
            }
            path.map(|p| (name, server.load(p)))
        })
        .collect();
    commands.insert_resource(Sounds {
        clips,
        next_pitch: 0,
        rng: rand::SeedableRng::seed_from_u64(9),
    });
}

/// How loud a sound of `volume` at `at` is at the listener: silent past
/// `HEARING × volume` (Chebyshev, as `isPositionHearable`), fading to it.
pub fn heard(volume: f32, at: Option<Vec2>, listener: Vec2) -> f32 {
    let Some(at) = at else {
        return volume;
    };
    let reach = HEARING * volume;
    let d = (at - listener).abs().max_element();
    if reach <= 0.0 || d >= reach {
        0.0
    } else {
        volume * (1.0 - d / reach)
    }
}

fn listener(cameras: &Query<&MainCamera>) -> Vec2 {
    cameras.iter().next().map_or(Vec2::ZERO, |c| c.focus)
}

fn play_sounds(
    mut commands: Commands,
    mut requests: MessageReader<PlaySound>,
    sounds: Res<Sounds>,
    cameras: Query<&MainCamera>,
) {
    let ear = listener(&cameras);
    for r in requests.read() {
        let gain = heard(r.volume, r.at, ear);
        let Some(clip) = sounds.clip(r.name).filter(|_| gain > 0.0) else {
            continue;
        };
        commands.spawn((
            Name::new(format!("Sound {}", r.name)),
            AudioPlayer::new(clip),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(gain),
                speed: 2f32.powf(r.pitch.clamp(-1.0, 1.0)),
                ..default()
            },
        ));
    }
}

fn impact_sounds(mut impacts: MessageReader<CarImpact>, mut sounds: ResMut<Sounds>, mut play: MessageWriter<PlaySound>) {
    for impact in impacts.read() {
        let rng = &mut sounds.rng;
        let name = ["CarBom0", "CarBom1", "CarBom2"][rng.random_range(0..3)];
        play.write(PlaySound {
            name,
            at: Some(impact.at),
            pitch: rng.random_range(-0.5..0.5),
            volume: impact.strength * 0.6 + 0.4,
        });
    }
}

fn blast_sounds(mut blasts: MessageReader<Explosion>, mut sounds: ResMut<Sounds>, mut play: MessageWriter<PlaySound>) {
    for blast in blasts.read() {
        let name = if sounds.rng.random_bool(0.7) { "RocketDestroy" } else { "Explosion" };
        let pitch = PITCH_ROUND[sounds.next_pitch];
        sounds.next_pitch = (sounds.next_pitch + 1) % PITCH_ROUND.len();
        play.write(PlaySound {
            name,
            at: Some(blast.at),
            pitch,
            volume: 1.0,
        });
    }
}

fn damage_sounds(
    mut hits: MessageReader<Hit>,
    players: Query<(&Health, &Transform), With<LocalPlayer>>,
    mut sounds: ResMut<Sounds>,
    mut play: MessageWriter<PlaySound>,
) {
    for hit in hits.read() {
        let Ok((health, t)) = players.get(hit.target) else {
            continue;
        };
        // Runs after the hits are taken: health is already down.
        let name = if health.hp <= 0 {
            "Accident"
        } else {
            ["CarDamaged0", "CarDamaged1", "CarDamaged2"][sounds.rng.random_range(0..3)]
        };
        play.write(PlaySound {
            name,
            at: Some(t.translation.truncate()),
            pitch: 0.0,
            volume: 1.0,
        });
    }
}

fn coin_sounds(mut coins: MessageReader<CoinCollected>, mut play: MessageWriter<PlaySound>) {
    for _ in coins.read() {
        play.write(PlaySound {
            name: "CoinCollected",
            at: None,
            pitch: 0.0,
            volume: 1.0,
        });
    }
}

fn finish_sounds(mut changes: MessageReader<ChangeMap>, mut play: MessageWriter<PlaySound>) {
    for change in changes.read() {
        if matches!(change, ChangeMap::Next) {
            play.write(PlaySound {
                name: "EndLevel",
                at: None,
                pitch: 0.0,
                volume: 1.0,
            });
        }
    }
}

/// One looping sound: started paused on first use, then steered.
fn steer_loop(
    commands: &mut Commands,
    player: &mut Option<Entity>,
    clip: Option<Handle<AudioSource>>,
    sinks: &mut Query<&mut AudioSink>,
    on: Option<(f32, f32)>,
) {
    let entity = match *player {
        Some(e) => e,
        None => {
            let (Some(clip), Some(_)) = (clip, on) else {
                return;
            };
            let e = commands
                .spawn((Name::new("Car sound"), MapEntity, AudioPlayer::new(clip), PlaybackSettings::LOOP))
                .id();
            *player = Some(e);
            return;
        }
    };
    let Ok(mut sink) = sinks.get_mut(entity) else {
        return;
    };
    match on {
        Some((pitch, volume)) => {
            sink.set_speed(2f32.powf(pitch.clamp(-1.0, 1.0)));
            sink.set_volume(Volume::Linear(volume));
            if sink.is_paused() {
                sink.play();
            }
        }
        None => sink.pause(),
    }
}

#[allow(clippy::type_complexity)]
fn car_loops(
    mut commands: Commands,
    time: Res<Time>,
    (map, state): (Option<Res<LoadedMap>>, Res<State<GameState>>),
    mut sounds: ResMut<Sounds>,
    cameras: Query<&MainCamera>,
    mut cars: Query<(&Car, &mut CarSounds, &Transform, &LinearVelocity, Option<&PlayerIntent>)>,
    mut sinks: Query<&mut AudioSink>,
) {
    let ear = listener(&cameras);
    let playing = map.is_some() && *state.get() == GameState::Playing;
    let dt = time.delta_secs();
    for (car, mut s, t, v, intent) in &mut cars {
        let at = Some(t.translation.truncate());
        let stick = intent.map_or(0.0, |i| i.move_dir.length());
        let top = car.params.max_speed;

        // Engine: only while the stick is held, and the car is driven.
        let engine = (playing && car.drive.steering && intent.is_some()).then(|| {
            let volume = (0.05 + 0.95 * stick).min(1.0);
            ((v.0.length() / top).min(1.0), heard(volume, at, ear))
        });
        let clip = s.engine.and_then(|e| sounds.clip(e));
        let mut player = s.engine_player;
        steer_loop(&mut commands, &mut player, clip, &mut sinks, engine);
        s.engine_player = player;

        // Tyres.
        let speed2 = (v.0.length_squared() / (top * top)).min(1.0);
        let target = if playing && car.drive.skidding && intent.is_some() {
            let sideways = (FRAC_PI_4..3.0 * FRAC_PI_4).contains(&car.drive.wheel_angle.abs());
            if sideways { speed2 * 0.9 } else { 0.4 + 0.6 * speed2 }
        } else {
            0.0
        };
        s.squeal += (target - s.squeal) * (3.0 * dt).min(1.0);
        let squeal = (s.squeal > 0.15).then(|| (s.wheels_pitch, heard(s.squeal, at, ear)));
        if squeal.is_none() && s.squeal_clip_should_change() {
            s.squeal_clip = sounds.rng.random_range(0..2);
            if let Some(e) = s.wheel_player.take() {
                commands.entity(e).despawn();
            }
        }
        let clip = sounds.clip(["wheels", "Wheels1"][s.squeal_clip]);
        let mut player = s.wheel_player;
        steer_loop(&mut commands, &mut player, clip, &mut sinks, squeal);
        s.wheel_player = player;
    }
}

impl CarSounds {
    /// A new squeal is picked once the last one has stopped.
    fn squeal_clip_should_change(&self) -> bool {
        self.wheel_player.is_some()
    }
}
