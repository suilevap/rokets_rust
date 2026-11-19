//! Textures as XNA added them.
//!
//! The C# content build premultiplied every sprite (`PremultiplyAlpha=True`:
//! colour times alpha in the file's own numbers), and the additive pass
//! (`DefaultWorldAdditive`, `BlendState.Additive`) multiplied by the alpha
//! once more (`SourceAlpha, One`). So a soft texture's edge added
//! `colour × alpha²`. Bevy's `AlphaMode::Add` multiplies by the alpha once.
//! For additive sprites (particles, the aim line) we draw a premultiplied
//! copy of the texture, and Bevy's multiply makes it `alpha²` again; without
//! it soft glows came out wider and brighter than in C#.
//!
//! Blended sprites keep the plain texture: XNA's premultiplied blend
//! (`One, InvSourceAlpha`) of a premultiplied texture is what Bevy's
//! straight blend does.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Premultiplied copies, by the texture they are made from.
#[derive(Resource, Default)]
pub struct Premultiplied(HashMap<AssetId<Image>, Handle<Image>>);

impl Premultiplied {
    /// The premultiplied copy of `image`, made on first use; the image
    /// itself while it is not loaded or not 8-bit RGBA.
    pub fn of(&mut self, image: &Handle<Image>, images: &mut Assets<Image>) -> Handle<Image> {
        if let Some(copy) = self.0.get(&image.id()) {
            return copy.clone();
        }
        let Some(source) = images.get(image) else {
            return image.clone();
        };
        let rgba8 = matches!(
            source.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
        );
        let Some(data) = source.data.as_ref().filter(|_| rgba8) else {
            return image.clone();
        };
        let mut copy = source.clone();
        copy.data = Some(premultiply(data));
        copy.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let handle = images.add(copy);
        self.0.insert(image.id(), handle.clone());
        handle
    }
}

/// RGBA bytes with each colour times its alpha, in the bytes' own numbers
/// (as XNA's content processor did, on sRGB values).
pub fn premultiply(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as u32;
            let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            [m(p[0]), m(p[1]), m(p[2]), p[3]]
        })
        .collect()
}
