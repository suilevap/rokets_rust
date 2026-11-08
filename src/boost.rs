//! Boost pads (`Accelerator` in C#): whatever body touches a [`Booster`]
//! is thrown along its facing, `speed` faster (an impulse of speed × mass),
//! and the booster plays its flash over its sprites.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::animation::Pulse;
use crate::game::GameState;
use crate::render::animate::TintPulse;

/// Throws touching bodies along this entity's facing.
#[derive(Component, Debug)]
pub struct Booster {
    /// Speed added, units per second.
    pub speed: f32,
    /// Played over the entity's sprites after each boost.
    pub flash: Option<Pulse>,
}

pub struct BoostPlugin;

impl Plugin for BoostPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, boost_on_contact.run_if(in_state(GameState::Playing)));
    }
}

fn boost_on_contact(
    mut commands: Commands,
    mut started: MessageReader<CollisionStart>,
    pads: Query<(&Booster, &Transform)>,
    mut bodies: Query<(&RigidBody, &mut LinearVelocity)>,
) {
    for event in started.read() {
        for (pad, other) in [(event.collider1, event.body2), (event.collider2, event.body1)] {
            let (Ok((booster, transform)), Some(other)) = (pads.get(pad), other) else {
                continue;
            };
            if let Ok((RigidBody::Dynamic, mut v)) = bodies.get_mut(other) {
                let facing = (transform.rotation * Vec3::X).truncate();
                v.0 += facing * booster.speed;
                if let Some(flash) = booster.flash {
                    commands.entity(pad).insert(TintPulse::start(flash));
                }
            }
        }
    }
}
