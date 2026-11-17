//! Force feedback: what the player feels, for now a jolt of the camera
//! (`Control.ForceFeedback`, `Camera2D.Shake`). Vibration of phones and
//! gamepads is still to come (see the backlog).
//!
//! - **Head-on hits**: when the car runs into something hard
//!   (`Car.OnCollide`: blow over 0.4, its motion within 45° of straight at
//!   it), by `(blow − 0.4) × 1.5`.
//! - **Damage** to the player's car: by the share of its health lost.
//! - **Blasts** in sight (`CreateExplosion`): 0.3.

use bevy::prelude::*;

use crate::car::impact::CarImpact;
use crate::controls::LocalPlayer;
use crate::explosion::{Explosion, Health, Hit};
use crate::game::GameState;
use crate::render::camera::{CameraShake, MainCamera};

/// Blows weaker than this are not felt (`pow > 0.4f`).
const FELT_BLOW: f32 = 0.4;
/// How near the view's centre a blast must be to shake it, world units
/// (C# asked whether it was in view, within twice its range).
const BLAST_IN_SIGHT: f32 = 20.0;

pub struct FeedbackPlugin;

impl Plugin for FeedbackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (hit_jolts, damage_jolts, blast_jolts).run_if(in_state(GameState::Playing)));
    }
}

fn hit_jolts(
    mut impacts: MessageReader<CarImpact>,
    players: Query<(), With<LocalPlayer>>,
    mut shake: ResMut<CameraShake>,
) {
    for impact in impacts.read() {
        let head_on = impact.velocity.normalize_or_zero().dot(impact.away) < -0.7;
        if impact.strength > FELT_BLOW && head_on && players.contains(impact.car) {
            shake.add((impact.strength - FELT_BLOW) * 1.5);
        }
    }
}

fn damage_jolts(
    mut hits: MessageReader<Hit>,
    players: Query<&Health, With<LocalPlayer>>,
    mut shake: ResMut<CameraShake>,
) {
    for hit in hits.read() {
        if let Ok(health) = players.get(hit.target) {
            shake.add((hit.damage as f32 / health.max as f32).min(1.0));
        }
    }
}

fn blast_jolts(
    mut explosions: MessageReader<Explosion>,
    cameras: Query<&MainCamera>,
    mut shake: ResMut<CameraShake>,
) {
    for ex in explosions.read() {
        if cameras.iter().any(|c| c.focus.distance(ex.at) < BLAST_IN_SIGHT + ex.blast.range * 2.0) {
            shake.add(0.3);
        }
    }
}
