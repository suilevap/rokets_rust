//! Pickups: the player collects a [`Pickup`] (a gear) by driving into it.
//! The count is shown in the debug text until the HUD exists.

use bevy::prelude::*;

use crate::controls::LocalPlayer;
use crate::game::GameState;

/// Collected by driving into it.
#[derive(Component, Debug)]
pub struct Pickup {
    /// Pick-up distance from the car's centre, in world units.
    pub radius: f32,
}

/// Pickups collected on this map, of how many.
#[derive(Resource, Debug, Default)]
pub struct CoinCount {
    pub collected: u32,
    pub total: u32,
}

#[derive(Message, Debug)]
pub struct CoinCollected;

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CoinCount>()
            .add_message::<CoinCollected>()
            .add_systems(OnEnter(GameState::Loading), reset_count)
            .add_systems(Update, (count_new_pickups, collect).run_if(in_state(GameState::Playing)));
    }
}

fn reset_count(mut count: ResMut<CoinCount>) {
    *count = CoinCount::default();
}

fn count_new_pickups(added: Query<(), Added<Pickup>>, mut count: ResMut<CoinCount>) {
    let n = added.iter().count() as u32;
    if n > 0 {
        count.total += n;
    }
}

fn collect(
    mut commands: Commands,
    players: Query<&Transform, (With<LocalPlayer>, Without<Pickup>)>,
    pickups: Query<(Entity, &Transform, &Pickup)>,
    mut count: ResMut<CoinCount>,
    mut collected: MessageWriter<CoinCollected>,
) {
    for player in &players {
        let p = player.translation.truncate();
        for (entity, t, pickup) in &pickups {
            // The car's size is about one cell; half of it plus the pickup's.
            if t.translation.truncate().distance(p) < pickup.radius + 1.0 {
                commands.entity(entity).despawn();
                count.collected += 1;
                collected.write(CoinCollected);
            }
        }
    }
}
