//! TileRider in Rust and Bevy. See `ARCHITECTURE.md` for the layout.

pub mod actions;
pub mod animation;
pub mod assets;
pub mod boost;
pub mod button;
pub mod car;
pub mod content;
pub mod controls;
pub mod door;
pub mod explosion;
pub mod feedback;
pub mod game;
pub mod game_mode;
pub mod hud;
pub mod info;
pub mod light;
pub mod map;
pub mod markers;
pub mod particles;
pub mod pickups;
pub mod platform;
pub mod render;
pub mod script;
pub mod sound;
pub mod teleport;
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
                pickups::PickupsPlugin,
                (boost::BoostPlugin, explosion::ExplosionPlugin, game_mode::GameModePlugin, feedback::FeedbackPlugin, sound::SoundPlugin),
                map::spawn::MapEntitiesPlugin,
                render::spin::SpinPlugin,
                render::animate::AnimatePlugin,
                (triggers::TriggersPlugin, actions::ActionsPlugin, door::DoorPlugin, button::ButtonPlugin, teleport::TeleportPlugin, platform::PlatformPlugin, info::InfoPlugin, markers::MarkersPlugin),
                hud::HudPlugin,
                script::ScriptPlugin,
            ));
    }
}
