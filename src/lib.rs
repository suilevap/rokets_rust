//! TileRider in Rust and Bevy. See `ARCHITECTURE.md` for the layout.

pub mod assets;
pub mod car;
pub mod content;
pub mod controls;
pub mod game;
pub mod hud;
pub mod light;
pub mod map;
pub mod pickups;
pub mod render;
pub mod shadow;
pub mod triggers;
pub mod units;

use avian2d::prelude::*;
use bevy::prelude::*;

/// Everything the game adds to a Bevy app (with `DefaultPlugins` already added).
pub struct TileRiderPlugin {
    pub map: String,
}

impl Plugin for TileRiderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(game::CurrentMapName(self.map.clone()))
            // The C# game stepped physics and the car at 60 Hz.
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(Gravity::ZERO)
            .add_plugins((
                PhysicsPlugins::default(),
                assets::ContentAssetsPlugin,
                game::GamePlugin,
                map::colliders::WallCollidersPlugin,
                render::RenderPlugin,
                controls::ControlsPlugin,
                car::CarPlugin,
                shadow::ShadowPlugin,
                pickups::PickupsPlugin,
                triggers::TriggersPlugin,
                hud::HudPlugin,
            ));
    }
}
