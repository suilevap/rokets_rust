//! [`Spin`]: an entity that turns on the spot (gears).

use bevy::prelude::*;

/// Radians per second.
#[derive(Component, Debug, Clone, Copy)]
pub struct Spin(pub f32);

pub struct SpinPlugin;

impl Plugin for SpinPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spin);
    }
}

fn spin(time: Res<Time>, mut spinning: Query<(&Spin, &mut Transform)>) {
    for (s, mut t) in &mut spinning {
        t.rotate_z(s.0 * time.delta_secs());
    }
}
