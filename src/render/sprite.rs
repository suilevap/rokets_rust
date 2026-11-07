//! Sprites as flat textured quads in the 3D world, tinted by the baked
//! light where they stand (the C# game multiplied everything under the
//! light layer by it).
//!
//! Sprite images are loaded with the map (see `game.rs`), so their sizes
//! are known when entities spawn: a sprite is `pixels × scale / 32` world
//! units, like `ConvertUnits.ToSimUnits(view.Width) × scale`.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::game::LoadedMap;
use crate::units::px;

#[derive(Clone, Debug)]
pub struct SpriteInfo {
    pub image: Handle<Image>,
    /// Size in the C# game's pixels.
    pub size: Vec2,
}

/// The sprites loaded for the current map, by lower-case content name.
#[derive(Clone, Debug, Default)]
pub struct SpriteSet(HashMap<String, SpriteInfo>);

impl SpriteSet {
    pub fn insert(&mut self, name: &str, info: SpriteInfo) {
        self.0.insert(key(name), info);
    }

    pub fn get(&self, name: &str) -> Option<&SpriteInfo> {
        self.0.get(&key(name))
    }
}

fn key(name: &str) -> String {
    name.trim().replace('/', "\\").to_ascii_lowercase()
}

/// How a sprite is coloured each frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct SpriteTint {
    /// sRGB colour (alpha = opacity) multiplied with the texture.
    pub color: [f32; 4],
    /// Multiply by the light map at the sprite's position.
    pub lit: bool,
}

/// One quad mesh shared by every sprite.
#[derive(Resource)]
pub struct UnitQuad(pub Handle<Mesh>);

pub struct SpritePlugin;

impl Plugin for SpritePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_unit_quad).add_systems(
            PostUpdate,
            tint_sprites
                .after(TransformSystems::Propagate)
                .run_if(resource_exists::<LoadedMap>),
        );
    }
}

fn make_unit_quad(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(UnitQuad(meshes.add(Rectangle::new(1.0, 1.0))));
}

/// Components for a sprite quad: `scale` is the entity's scale from the
/// XML, `z` its height above the ground plane.
pub fn sprite_bundle(
    info: &SpriteInfo,
    scale: f32,
    z: f32,
    tint: SpriteTint,
    quad: &UnitQuad,
    materials: &mut Assets<StandardMaterial>,
) -> impl Bundle {
    let size = info.size * px(1.0) * scale;
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(tint.color[0], tint.color[1], tint.color[2], tint.color[3]),
        base_color_texture: Some(info.image.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    (
        Mesh3d(quad.0.clone()),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, z).with_scale(size.extend(1.0)),
        tint,
    )
}

fn tint_sprites(
    map: Res<LoadedMap>,
    sprites: Query<(&SpriteTint, &GlobalTransform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (tint, transform, material) in &sprites {
        let [r, g, b, a] = tint.color;
        let l = if tint.lit {
            map.light_at(transform.translation().truncate())
        } else {
            [1.0; 3]
        };
        // Black overlays (shadows) darken by `alpha` in sRGB, as XNA blended;
        // Bevy blends in linear light, where the same alpha darkens less.
        let a = if r == 0.0 && g == 0.0 && b == 0.0 {
            1.0 - (1.0 - a).powf(2.2)
        } else {
            a
        };
        let color = Color::srgba(r * l[0], g * l[1], b * l[2], a);
        // Only touch materials whose colour changes: a write re-uploads it.
        if materials.get(&material.0).is_some_and(|m| m.base_color != color)
            && let Some(mut mat) = materials.get_mut(&material.0)
        {
            mat.base_color = color;
        }
    }
}
