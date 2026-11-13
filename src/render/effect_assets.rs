//! Renderer-owned textures shared across maps. Paths and sampling policy
//! live here; effect systems use typed handles rather than sprite names.

use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;

#[derive(Resource)]
pub(super) struct EffectTextures {
    pub dirty_glass: Handle<Image>,
    pub dark_corners: Handle<Image>,
    pub shadow_gradient: Handle<Image>,
    pub contact_gradient: Handle<Image>,
}

impl FromWorld for EffectTextures {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let linear = |path: &'static str| {
            server
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    settings.sampler = ImageSampler::linear();
                })
                .load(path)
        };
        Self {
            dirty_glass: linear("Sprites/DirtyGlass.png"),
            dark_corners: linear("Sprites/DarkCorners.png"),
            shadow_gradient: linear("Sprites/Gradient4Pixel.png"),
            contact_gradient: linear("Sprites/Gradient4PixelUniform.png"),
        }
    }
}
