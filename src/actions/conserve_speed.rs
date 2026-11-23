//! `AddConserveMaxReachedSpeedComponent` (`ConserveMaxReachedSpeedComponent`):
//! a body that never gets slower than the fastest it has been.
//!
//! C# origin: `ActionRunner.AddConserveMaxReachedSpeedComponentRun` and the
//! `ConserveMaxReachedSpeed` entity component, added in February 2014
//! (commit `aa6d9e47`) for the pong level: `DoorsDream1`'s ball (3003, and
//! its two clones 300301 and 300302) runs it from its `ActionOnCreate`
//! sequence just before its starting kick, so that a ball bounced off the
//! walls, the bats and the other balls (friction, damping and imperfect
//! bounces all take speed) keeps playing as fast as it was kicked, and
//! every hit the bats add stays. No other map uses it.
//!
//! What the component does, every update of an active entity: it reads the
//! speed (the length of the linear velocity). Slower than the highest seen
//! so far, the velocity is scaled back up to that speed, direction kept.
//! Not slower, the highest becomes this speed (it starts at 0, so the
//! first update takes the speed the kick gave). It runs in `FixedUpdate`,
//! before the physics step: it sees what the last step's collisions left
//! and the next step moves with the restored speed, as C# did (its
//! entities update, then `PhysicsManager` steps).
//!
//! Branches of the C#, all ported:
//! - the target is `GetEntity(ItemId, itemId)`: an `ItemId` of 0 or less
//!   means whoever set the action off (`Run::target`); a missing target
//!   does nothing (the action fails);
//! - `curSpeed < _maxSpeed`: scale the velocity by `max / cur`;
//! - otherwise: remember `curSpeed` as the highest;
//! - `Entity.Update` runs components only `if (IsActive)`: a deactivated
//!   entity ([`Inactive`]) is left alone, and its highest speed is kept
//!   for when it is back (a `DeactivateEntity(3003)` on `DoorsDream1`).
//!
//! Not ported:
//! - the second component C# makes when the action runs twice on one
//!   entity: both would hold the same highest speed from then on, so
//!   running it again here changes nothing (the first one's highest speed
//!   stays);
//! - a stopped body (speed 0) after a speed was reached: C# multiplies a
//!   zero velocity by `max / 0`, giving NaN, which breaks the body. Here
//!   a body with no velocity is left still (there is no direction to
//!   restore it along); the pong ball never stops;
//! - a target with no physics body: C# would throw in the next update; the
//!   action fails here.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{AddMapAction, Inactive, MapIds, Run};
use crate::content::actions::defs::AddConserveMaxReachedSpeedComponent;
use crate::game::GameState;

/// Keeps a body at the highest speed it has reached
/// (`ConserveMaxReachedSpeed`).
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct ConserveMaxReachedSpeed {
    /// The highest speed seen so far (`_maxSpeed`), in world units a second.
    pub max_speed: f32,
}

impl ConserveMaxReachedSpeed {
    /// `ConserveMaxReachedSpeed.Update`: the velocity the body gets for
    /// the one it has, remembering the highest speed.
    pub fn conserve(&mut self, velocity: Vec2) -> Vec2 {
        let speed = velocity.length();
        if speed < self.max_speed {
            // C# divides by zero here (NaN); a body that stopped stays so.
            if speed == 0.0 {
                return velocity;
            }
            velocity * (self.max_speed / speed)
        } else {
            self.max_speed = speed;
            velocity
        }
    }
}

pub struct ConserveSpeedPlugin;

impl Plugin for ConserveSpeedPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AddConserveMaxReachedSpeedComponent, _>(add).add_systems(
            FixedUpdate,
            conserve.before(super::ActionSystems).run_if(in_state(GameState::Playing)),
        );
    }
}

/// `AddConserveMaxReachedSpeedComponentRun`.
fn add(
    In(run): In<Run<AddConserveMaxReachedSpeedComponent>>,
    ids: MapIds,
    bodies: Query<Has<ConserveMaxReachedSpeed>, With<LinearVelocity>>,
    mut commands: Commands,
) -> bool {
    let Some(e) = ids.entity(run.target(run.data.item_id)) else {
        return false;
    };
    match bodies.get(e) {
        Ok(true) => {}
        Ok(false) => {
            commands.entity(e).insert(ConserveMaxReachedSpeed::default());
        }
        Err(_) => return false,
    }
    true
}

fn conserve(mut bodies: Query<(&mut ConserveMaxReachedSpeed, &mut LinearVelocity), Without<Inactive>>) {
    for (mut component, mut velocity) in &mut bodies {
        let kept = component.conserve(velocity.0);
        if kept != velocity.0 {
            velocity.0 = kept;
        }
    }
}
