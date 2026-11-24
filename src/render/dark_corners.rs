//! `DarkCorners`: the vignette, the picture darkening towards its corners.
//!
//! C# origin: `World.ViewDarkCorners` (2013, `Exp: Draw Dark corners`; the
//! sprite was redrawn in `version for Lesha prepared`). It adds one
//! `DefaultUI` item, drawn under the interface of every map: the `DarkCorners`
//! sprite (80 x 48, alpha only, 0 in the middle, up to 0.64 in the corners)
//! without its outer pixel row, stretched over the whole view and tinted
//! black, so each pixel is multiplied by one minus the sprite's alpha.
//!
//! Here it is a full-screen interface image under the HUD. XNA multiplied
//! sRGB numbers; blending in Bevy multiplies linear light, which darkens
//! much less, so the alpha is raised to what makes the sRGB number come out
//! as `number × (1 − alpha)` ([`vignette_pixels`]).
//!
//! Not ported:
//! - `Settings.DarkCornersEnabled` (a constant, true) and the
//!   `Settings.DrawEnvironment` check of the screenshot without environment
//!   (`#if WINDOWS`, a debug switch).
//! - The sprite's `LinearClamp` and the source rectangle's half-pixel
//!   handling: the image is cropped by one pixel on the CPU and sampled
//!   linearly.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game::{GameState, MapEntity};

/// Under every other interface node (the HUD is at 80, hints at 90).
const Z: i32 = -10;
/// The gamma of the sRGB numbers XNA multiplied.
const GAMMA: f32 = 2.2;

/// The vignette's black-and-alpha pixels from the `DarkCorners` sprite's
/// RGBA bytes: the outer pixel row dropped (`new Rectangle(1, 1, width - 2,
/// height - 2)`), the colour black (`Color.Black` tint) and the alpha `a`
/// replaced by `1 − (1 − a)^2.2`, so that multiplying linear light by
/// `1 − a'` multiplies the sRGB numbers by `1 − a`. Returns `(width,
/// height, RGBA bytes)`.
pub fn vignette_pixels(rgba: &[u8], width: usize, height: usize) -> (usize, usize, Vec<u8>) {
    assert_eq!(rgba.len(), width * height * 4, "RGBA bytes of a {width} x {height} image");
    let (w, h) = (width.saturating_sub(2), height.saturating_sub(2));
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 1..1 + h {
        for x in 1..1 + w {
            let a = rgba[(y * width + x) * 4 + 3] as f32 / 255.0;
            let linear = 1.0 - (1.0 - a).powf(GAMMA);
            out.extend_from_slice(&[0, 0, 0, (linear * 255.0).round() as u8]);
        }
    }
    (w, h, out)
}

/// The vignette node.
#[derive(Component)]
struct Vignette;

/// The loaded sprite, and the vignette made from it once it is there.
#[derive(Resource)]
struct VignetteSprite {
    source: Handle<Image>,
    made: Option<Handle<Image>>,
}

pub struct DarkCornersPlugin;

impl Plugin for DarkCornersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sprite)
            .add_systems(Update, spawn_vignette.run_if(in_state(GameState::Playing)));
    }
}

fn load_sprite(mut commands: Commands, server: Res<AssetServer>) {
    let source = server
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| s.asset_usage = RenderAssetUsages::all())
        .load("Sprites/DarkCorners.png");
    commands.insert_resource(VignetteSprite { source, made: None });
}

/// Puts the vignette on screen when the map has none (it goes with the
/// map, as the interface does between levels) and the sprite is there.
fn spawn_vignette(
    mut commands: Commands,
    existing: Query<(), With<Vignette>>,
    sprite: Option<ResMut<VignetteSprite>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut sprite) = sprite else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    if sprite.made.is_none() {
        let Some(source) = images.get(&sprite.source) else {
            return;
        };
        let size = source.size();
        let Some(data) = source
            .data
            .as_ref()
            .filter(|d| d.len() == (size.x * size.y * 4) as usize)
        else {
            return;
        };
        let (w, h, pixels) = vignette_pixels(data, size.x as usize, size.y as usize);
        let mut image = Image::new(
            Extent3d {
                width: w as u32,
                height: h as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::linear();
        sprite.made = Some(images.add(image));
    }
    let Some(image) = sprite.made.clone() else {
        return;
    };
    commands.spawn((
        Name::new("Dark corners"),
        MapEntity,
        Vignette,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode {
            image,
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
        GlobalZIndex(Z),
    ));
}
