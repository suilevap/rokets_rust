//! Animated sprite colours: [`TintLoop`] repeats a colour track on a wave
//! of game time, [`TintPulse`] plays a one-shot blend over an entity's
//! sprites (its own and its children's) and then removes itself.
//!
//! Game time is Bevy's virtual time, so animations pause and slow down with
//! the game, like the C# `PhaseHelper` clock. All loops with the same wave
//! stay in step, as they did there.

use bevy::prelude::*;

use crate::animation::{ColorTrack, Pulse, Wave};
use crate::render::sprite::SpriteTint;

/// Repeats `track` over `wave`, setting the sprite's tint.
#[derive(Component, Debug, Clone)]
pub struct TintLoop {
    pub wave: Wave,
    pub track: ColorTrack,
}

/// A one-shot blend over this entity's sprite and its children's; it
/// overrides their loops while it plays.
#[derive(Component, Debug, Clone)]
pub struct TintPulse {
    pub pulse: Pulse,
    pub elapsed: f32,
}

impl TintPulse {
    pub fn start(pulse: Pulse) -> Self {
        TintPulse { pulse, elapsed: 0.0 }
    }
}

/// Runs before sprite tints are applied.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimateSet;

pub struct AnimatePlugin;

impl Plugin for AnimatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (play_loops, play_pulses).chain().in_set(AnimateSet));
    }
}

fn play_loops(time: Res<Time>, mut sprites: Query<(&TintLoop, &mut SpriteTint)>) {
    let t = time.elapsed_secs();
    for (anim, mut tint) in &mut sprites {
        let color = anim.track.sample(anim.wave.phase(t));
        if tint.color != color {
            tint.color = color;
        }
    }
}

fn play_pulses(
    mut commands: Commands,
    time: Res<Time>,
    mut pulses: Query<(Entity, &mut TintPulse, Option<&Children>)>,
    mut tints: Query<&mut SpriteTint>,
) {
    for (entity, mut pulse, children) in &mut pulses {
        pulse.elapsed += time.delta_secs();
        let Some(color) = pulse.pulse.sample(pulse.elapsed) else {
            commands.entity(entity).remove::<TintPulse>();
            continue;
        };
        let targets = std::iter::once(entity).chain(children.into_iter().flat_map(|c| c.iter()));
        for target in targets {
            if let Ok(mut tint) = tints.get_mut(target) {
                tint.color = color;
            }
        }
    }
}
