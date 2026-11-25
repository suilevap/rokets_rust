//! Additive glows: soft sprites added to the picture as light.
//!
//! C# origin: the additive pass (`DefaultWorldAdditive`,
//! `BlendState.Additive`) added `colour × alpha` of a texture the content
//! build had already premultiplied (`colour × alpha`), so a soft sprite's
//! edge added `colour × alpha²`, where alpha is the texture's alpha times
//! the tint's. Bevy's `AlphaMode::Add` adds `colour × alpha` once, which
//! made soft glows wider and brighter than in C#.
//!
//! The second factor is a falloff of the glow's edge, and here it is kept
//! in one place: [`GlowMaterial`] is the standard unlit material whose
//! alpha is squared before it is added, so a sprite draws its original
//! texture with its straight colour and alpha (nothing is premultiplied
//! on the CPU or squared by the caller). It cannot be folded into the
//! vertex colour, as the texture's alpha is per pixel.
//!
//! A colour's own alpha is a strength, as the colour channels are (C#'s
//! tints were premultiplied: the alpha was already in them), so it is
//! given as [`alpha_for`] its strength; a texture's and a particle's
//! alpha is the sprite's own and goes in as it is.
//!
//! Not ported: C# squared the alpha of sRGB bytes, and the result was
//! added in sRGB numbers; here it is the straight alpha that is squared
//! and the sum is in linear light (see `ARCHITECTURE.md`).

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// A standard material added as light, its alpha squared.
pub type GlowMaterial = ExtendedMaterial<StandardMaterial, Squared>;

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct Squared {}

impl MaterialExtension for Squared {
    fn fragment_shader() -> ShaderRef {
        AssetPath::from_path_buf(embedded_path!("glow.wesl"))
            .with_source("embedded")
            .into()
    }

    // The shader has the main pass only; an added sprite casts no shadow.
    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The alpha of a tint that adds `strength` of its colour (0 to 1) over
/// an opaque texture: the sum takes the alpha squared.
pub fn alpha_for(strength: f32) -> f32 {
    strength.max(0.0).sqrt()
}

/// An added, unlit, double-sided glow of the layer's depth bias, with
/// `base` (a colour and a texture) as it is drawn.
pub fn additive(base: StandardMaterial, depth_bias: f32) -> GlowMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            depth_bias,
            ..base
        },
        extension: Squared {},
    }
}

pub struct GlowPlugin;

impl Plugin for GlowPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "glow.wesl");
        app.add_plugins(MaterialPlugin::<GlowMaterial>::default());
    }
}
