//! Boost pads (`Accelerator` in C#): whatever body touches a [`Booster`]
//! is thrown along its facing at `speed` more (an impulse of speed × mass).
//! [`PadArrows`] draws such a pad as two arrows that light yellow in turn
//! every second and both flash white after a boost.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::game::GameState;
use crate::render::sprite::SpriteTint;

/// Seconds both arrows flash after a boost.
const FLASH_SECONDS: f32 = 1.0;

/// Throws touching bodies along this entity's facing.
#[derive(Component, Debug)]
pub struct Booster {
    /// Speed added, units per second.
    pub speed: f32,
}

/// Two arrows that blink in turn; `flash` > 0 after a boost.
#[derive(Component, Debug, Default)]
pub struct PadArrows {
    flash: f32,
}

/// One of a pad's arrows: 0 behind, 1 ahead.
#[derive(Component, Debug)]
pub struct PadArrow(pub usize);

pub struct BoostPlugin;

impl Plugin for BoostPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (boost_on_contact, blink_pad_arrows).run_if(in_state(GameState::Playing)),
        );
    }
}

fn boost_on_contact(
    mut started: MessageReader<CollisionStart>,
    mut pads: Query<(&Booster, &Transform, Option<&mut PadArrows>)>,
    mut bodies: Query<(&RigidBody, &mut LinearVelocity)>,
) {
    for event in started.read() {
        for (pad, other) in [(event.collider1, event.body2), (event.collider2, event.body1)] {
            let (Ok((booster, transform, arrows)), Some(other)) = (pads.get_mut(pad), other) else {
                continue;
            };
            if let Ok((RigidBody::Dynamic, mut v)) = bodies.get_mut(other) {
                let facing = (transform.rotation * Vec3::X).truncate();
                v.0 += facing * booster.speed;
                if let Some(mut arrows) = arrows {
                    arrows.flash = FLASH_SECONDS;
                }
            }
        }
    }
}

fn blink_pad_arrows(
    time: Res<Time>,
    mut pads: Query<(&mut PadArrows, &Children)>,
    mut arrows: Query<(&PadArrow, &mut SpriteTint)>,
) {
    const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
    const GRAY: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
    let phase = time.elapsed_secs() % 1.0;
    let lit_arrow = if phase < 0.4 {
        Some(0)
    } else if phase < 0.8 {
        Some(1)
    } else {
        None
    };
    for (mut pad, children) in &mut pads {
        pad.flash = (pad.flash - time.delta_secs()).max(0.0);
        for child in children.iter() {
            let Ok((arrow, mut tint)) = arrows.get_mut(child) else {
                continue;
            };
            let color = if pad.flash > 0.0 {
                let t = pad.flash / FLASH_SECONDS;
                std::array::from_fn(|i| GRAY[i] + (1.0 - GRAY[i]) * t)
            } else if lit_arrow == Some(arrow.0) {
                YELLOW
            } else {
                GRAY
            };
            if tint.color != color {
                tint.color = color;
            }
        }
    }
}
