//! Sound cues (`SoundManager`, `EffectManager`, the car's sound control):
//! what should be heard, worked out from game events, whether or not
//! anything plays it.
//!
//! - **One-shots** ([`PlaySound`] messages): a sound at a point, with a
//!   pitch in octaves (-1..1, as XNA's) and a volume.
//!   - **Hits** (`EffectManager.CollideEffect`): one of three bumps,
//!     louder the harder the blow, at a random pitch within half an octave.
//!   - **Blasts**: `RocketDestroy` (seven times in ten) or `Explosion`, at
//!     the next pitch of a fixed round (`PlayOnceWithNextPith`).
//!   - **Damage** to the player: one of three crunches; `Accident` when it
//!     is destroyed. **Gears**: `CoinCollected`. **Finish**: `EndLevel`.
//! - **Loops** on the car ([`CarSounds`]), updated every frame:
//!   - **Engine** (`Car.EngineSoundControl`): while the stick is held, the
//!     car's `EngineSound`, louder with the stick, higher with the speed
//!     (an octave up at top speed); silent otherwise.
//!   - **Tyres** (`Car.CreateWheelsTrailAndSound`): while the wheels slip
//!     the squeal eases (3/s) towards `0.4 + 0.6 × speed²` (`speed²` × 0.9
//!     when turning on the spot), heard while above 0.15; one of two
//!     squeals, picked anew each time it stops.
//!
//! Playing them is `playback`, behind the `sound` cargo feature (on by
//! default; `--no-default-features` builds a silent game without the
//! audio code). The WAVs come with the content (`Sounds/*.wav`, 2.6 MB).
//! `RUST_LOG=tilerider::sound=debug` lists the cues.
//!
//! In the browser, sound starts with the first touch, click or key press
//! (`web/index.html` resumes the audio then; browsers keep it off until
//! the page is used).

#[cfg(feature = "sound")]
mod playback;

use std::f32::consts::FRAC_PI_4;

use avian2d::prelude::LinearVelocity;
use bevy::prelude::*;
use rand::Rng;
use rand::rngs::SmallRng;

use crate::car::Car;
use crate::car::impact::CarImpact;
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::explosion::{Explosion, ExplosionSystems, Health, Hit};
use crate::game::{ChangeMap, GameState};
use crate::pickups::CoinCollected;

/// `SoundManager.CanHearRadius`: 1024 px.
pub const HEARING: f32 = 32.0;

/// Every sound a cue names (`Sounds/<name>.wav` in the C# content).
pub const NAMES: [&str; 19] = [
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
    "GButtonLight",
    "GButtonActivate",
    "GButtonRelease",
];

/// `SoundManager._pitchArray`: the round of pitches for repeated blasts.
const PITCH_ROUND: [f32; 9] = [0.0, 0.5, -0.25, 0.25, 0.75, -0.5, 0.5, 1.0, -1.0];

/// Play a sound once.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlaySound {
    pub name: &'static str,
    /// Where it happens; none: at the listener.
    pub at: Option<Vec2>,
    /// Octaves, -1..1.
    pub pitch: f32,
    /// 0..1.
    pub volume: f32,
}

/// A looping sound as it should sound now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopCue {
    pub name: &'static str,
    /// Octaves, -1..1.
    pub pitch: f32,
    /// 0..1.
    pub volume: f32,
}

/// The car's engine and tyres: what they should sound like this frame.
#[derive(Component, Debug)]
pub struct CarSounds {
    /// The car's `EngineSound`, if it has one.
    engine_name: Option<&'static str>,
    /// Octaves added to the tyres (`WheelsSoundPitch`).
    wheels_pitch: f32,
    squeal_level: f32,
    squeal_name: usize,
    pub engine: Option<LoopCue>,
    pub squeal: Option<LoopCue>,
}

impl CarSounds {
    pub fn new(engine: Option<&str>, wheels_pitch: f32) -> CarSounds {
        CarSounds {
            engine_name: engine.and_then(|e| NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(e))),
            wheels_pitch,
            squeal_level: 0.0,
            squeal_name: 0,
            engine: None,
            squeal: None,
        }
    }
}

/// How loud a sound of `volume` at `at` is for a listener at `ear`:
/// silent past `HEARING × volume` (Chebyshev, as `isPositionHearable`),
/// fading linearly to it. C# placed sounds in 3D; this has no panning.
pub fn heard(volume: f32, at: Option<Vec2>, ear: Vec2) -> f32 {
    let Some(at) = at else {
        return volume;
    };
    let reach = HEARING * volume;
    let d = (at - ear).abs().max_element();
    if reach <= 0.0 || d >= reach {
        0.0
    } else {
        volume * (1.0 - d / reach)
    }
}

#[derive(Resource)]
struct Cues {
    next_pitch: usize,
    rng: SmallRng,
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>()
            .insert_resource(Cues {
                next_pitch: 0,
                rng: rand::SeedableRng::seed_from_u64(9),
            })
            .add_systems(
                Update,
                (
                    (impact_sounds, blast_sounds, damage_sounds, coin_sounds, finish_sounds, car_loops),
                    log_cues,
                )
                    .chain()
                    .after(ExplosionSystems),
            );
        #[cfg(feature = "sound")]
        app.add_plugins(playback::PlaybackPlugin);
    }
}

fn log_cues(mut cues: MessageReader<PlaySound>) {
    for cue in cues.read() {
        debug!("sound {cue:?}");
    }
}

fn impact_sounds(mut impacts: MessageReader<CarImpact>, mut cues: ResMut<Cues>, mut play: MessageWriter<PlaySound>) {
    for impact in impacts.read() {
        let rng = &mut cues.rng;
        play.write(PlaySound {
            name: ["CarBom0", "CarBom1", "CarBom2"][rng.random_range(0..3)],
            at: Some(impact.at),
            pitch: rng.random_range(-0.5..0.5),
            volume: impact.strength * 0.6 + 0.4,
        });
    }
}

fn blast_sounds(mut blasts: MessageReader<Explosion>, mut cues: ResMut<Cues>, mut play: MessageWriter<PlaySound>) {
    for blast in blasts.read() {
        let name = if cues.rng.random_bool(0.7) { "RocketDestroy" } else { "Explosion" };
        let pitch = PITCH_ROUND[cues.next_pitch];
        cues.next_pitch = (cues.next_pitch + 1) % PITCH_ROUND.len();
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
    mut cues: ResMut<Cues>,
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
            ["CarDamaged0", "CarDamaged1", "CarDamaged2"][cues.rng.random_range(0..3)]
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

fn car_loops(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut cues: ResMut<Cues>,
    mut cars: Query<(&Car, &mut CarSounds, &LinearVelocity, Option<&PlayerIntent>)>,
) {
    let playing = *state.get() == GameState::Playing;
    let dt = time.delta_secs();
    for (car, mut s, v, intent) in &mut cars {
        let driven = playing && intent.is_some();
        let top = car.params.max_speed;

        // Engine: while the stick is held.
        let stick = intent.map_or(0.0, |i| i.move_dir.length());
        s.engine = s.engine_name.filter(|_| driven && car.drive.steering).map(|name| LoopCue {
            name,
            pitch: (v.0.length() / top).min(1.0),
            volume: (0.05 + 0.95 * stick).min(1.0),
        });

        // Tyres: eased towards the slip's loudness.
        let speed2 = (v.0.length_squared() / (top * top)).min(1.0);
        let target = if driven && car.drive.skidding {
            let sideways = (FRAC_PI_4..3.0 * FRAC_PI_4).contains(&car.drive.wheel_angle.abs());
            if sideways { speed2 * 0.9 } else { 0.4 + 0.6 * speed2 }
        } else {
            0.0
        };
        s.squeal_level += (target - s.squeal_level) * (3.0 * dt).min(1.0);
        let was = s.squeal.is_some();
        s.squeal = (s.squeal_level > 0.15).then(|| LoopCue {
            name: ["wheels", "Wheels1"][s.squeal_name],
            pitch: s.wheels_pitch,
            volume: s.squeal_level,
        });
        if was && s.squeal.is_none() {
            s.squeal_name = cues.rng.random_range(0..2);
        }
    }
}
