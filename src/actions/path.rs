//! `AddPathComponent`: a dotted route line drawn on the floor, to show the
//! player where something goes (a door's key, a turret's target, the way a
//! truck is led). It is purely visual and changes nothing in the game.
//!
//! Used by `Derby`, `Escort2`, `JunkYard`, `Revenge`, `TakeCover`,
//! `TheLastStand` and `UnderPressure`, each at map start, one line of two
//! to three points (cells) per target (`ItemId`), white, on the floor,
//! 5 pixels wide.
//!
//! C# origin: `ActionRunner.AddPathComponentRun`, `Entities/Components/Path`,
//! `Render/DottedSpritePath` (added 2014-02-13, "Change PathComponent to use
//! dottedpath": it replaced a solid `SpritePath` so a route reads as a
//! marching trail rather than a wall of light).
//!
//! The line:
//!
//! - Each leg of the path is a strip of the one-pixel-high sprite
//!   `Special\DottedLine` (128 texels, 3 lit then 1 dark, so a period of 4),
//!   starting at the leg's first point and running to its second, `Width`
//!   pixels thick, centred on the leg, `Color` times the texture, blended
//!   additively over the lit world (`DefaultWorldAdditive`).
//! - A texel is 4 pixels (1/8 unit) long. A leg is drawn from a whole number
//!   of texels, cut short of its end by the part of a texel that is left
//!   over. The strip the sprite gives is 124 texels (the last period is
//!   kept for the animation): when the longest leg is longer than that, the
//!   texels of *all* legs grow in the same proportion, so the dots stay of
//!   one size along the whole path.
//! - The dots march along the line, once a period a second: the strip
//!   starts `int((1 − Sawtooth_Period1000) × 4)` texels into the sprite.
//! - The line stays where the map put it. It does not follow the entity,
//!   which only owns it: when the entity is gone the line is gone
//!   (`Path.Destroy`). A second `AddPathComponent` for the same entity
//!   replaces its line (`SetPath`).
//!
//! Not ported:
//! - `IsDottedLine = false` (the solid line of the same class) and the
//!   static `DottedSpritePath.Draw(p1, p2, …)` for two points: no action
//!   sets either (the action always draws dots; buttons used the static one).
//! - The `#if DEBUG` translucent red outline of the path (its renderable was
//!   never added, so it did not draw even in C#).
//! - `Drawer.Camera.IsInView` culling: Bevy culls the quads itself.
//! - `Depth` below the floor (`< 0`, "down"): the line stays on the floor.
//!   Above it, `Depth` units up as `AttachAdditionalView`'s.
//! - A path of no points: C# threw when sizing its lists; here nothing is
//!   drawn and the action fails. One point draws nothing, as in C#.
//! - A leg that is shorter than a texel draws nothing (C# gave it a rect
//!   of 0 width, which draws nothing either).

use bevy::math::Affine2;
use bevy::prelude::*;

use super::{AddMapAction, MapIds, Run};
use crate::animation::Wave;
use crate::content::actions::defs::AddPathComponent;
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::render::layers::Layer;
use crate::render::particles::FlashBoost;
use crate::render::glow::{GlowMaterial, additive, alpha_for};
use crate::render::sprite::UnitQuad;
use crate::units::PX_PER_UNIT;

/// The dotted strip: 128 texels wide, one high.
const DOTTED: &str = "Special\\DottedLine";

/// `DottedSpritePath._period`: texels from one dot to the next.
const PERIOD: u32 = 4;

/// `DottedSpritePath._normalScaleX`: a texel's length, world units
/// (4 C# pixels).
const TEXEL: f32 = 4.0 / PX_PER_UNIT;

pub struct PathActionsPlugin;

impl Plugin for PathActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_map_action::<AddPathComponent, _>(add_path)
            .add_action_sprites::<AddPathComponent>(&[DOTTED])
            .add_systems(Update, (drop_orphans, march).chain().run_if(in_state(GameState::Playing)));
    }
}

/// One leg of a path, as a strip of the dotted sprite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub start: Vec2,
    /// World angle of the leg, radians.
    pub angle: f32,
    /// Texels of the sprite the leg shows.
    pub texels: u32,
    /// How long one texel is drawn, world units.
    pub texel_length: f32,
}

impl Leg {
    /// How far the strip reaches along the leg, world units.
    pub fn length(&self) -> f32 {
        self.texels as f32 * self.texel_length
    }
}

/// The legs of a path (`DottedSpritePath.Init`), for a sprite `sprite_width`
/// texels wide. Legs of no length are left out, as are those under a texel.
pub fn legs(points: &[Vec2], sprite_width: u32) -> Vec<Leg> {
    if points.len() < 2 {
        return Vec::new();
    }
    let effective = sprite_width.saturating_sub(PERIOD) as f32;
    let scales: Vec<f32> = points.windows(2).map(|w| w[0].distance(w[1]) / (effective * TEXEL)).collect();
    // The longest leg sets how long every texel is, but never under 1.
    let max_scale = scales.iter().copied().fold(1.0, f32::max);
    let inverse = 1.0 / max_scale;
    points
        .windows(2)
        .zip(&scales)
        .filter_map(|(w, scale)| {
            let texels = (effective * (scale * inverse)) as u32;
            (texels > 0).then(|| Leg {
                start: w[0],
                angle: (w[1] - w[0]).to_angle(),
                texels,
                texel_length: TEXEL * max_scale,
            })
        })
        .collect()
}

/// How far into the sprite the strips start at `t` seconds of game time
/// (`(1 − Sawtooth_Period1000) × 4`, truncated): 3, 2, 1, 0 over a second,
/// so the dots march towards the end of each leg.
pub fn offset(t: f32) -> u32 {
    ((1.0 - Wave::Sawtooth { period: 1.0 }.phase(t)) * PERIOD as f32) as u32
}

/// A leg's quad: the sprite from texel `offset`, `leg.texels` of it.
pub fn uv(leg: &Leg, sprite_width: u32, offset: u32) -> Affine2 {
    let w = sprite_width as f32;
    Affine2::from_scale_angle_translation(Vec2::new(leg.texels as f32 / w, 1.0), 0.0, Vec2::new(offset as f32 / w, 0.0))
}

/// A leg of an entity's line.
#[derive(Component)]
struct DottedLeg {
    owner: Entity,
    leg: Leg,
    sprite_width: u32,
    /// The offset the material shows now.
    shown: u32,
}

fn add_path(
    In(run): In<Run<AddPathComponent>>,
    (ids, map): (MapIds, Res<LoadedMap>),
    (quad, flash, time): (Res<UnitQuad>, Res<FlashBoost>, Res<Time>),
    old: Query<(Entity, &DottedLeg)>,
    mut materials: ResMut<Assets<GlowMaterial>>,
    mut commands: Commands,
) -> bool {
    let d = &run.data;
    // No such entity: nothing to do, and C# counts it as run.
    let Some(owner) = ids.entity(run.target(d.item_id)) else {
        return true;
    };
    if d.path.is_empty() {
        return false;
    }
    let Some(info) = map.sprites.get(DOTTED) else {
        return false;
    };
    // `SetPath`: the new line takes the old one's place.
    for (e, leg) in &old {
        if leg.owner == owner {
            commands.entity(e).despawn();
        }
    }
    let width = info.size.x as u32;
    let points: Vec<Vec2> = d.path.iter().map(|p| map.world(p.x, p.y)).collect();
    let [r, g, b, a] = d.color.to_f32();
    let c = Color::srgba(r, g, b, a).to_linear();
    let color = Color::linear_rgba(c.red * flash.0, c.green * flash.0, c.blue * flash.0, alpha_for(c.alpha));
    let now = offset(time.elapsed_secs());
    let z = Layer::Glow.z() + d.depth.max(0.0);
    for leg in legs(&points, width) {
        let material = materials.add(additive(
            StandardMaterial {
                base_color: color,
                base_color_texture: Some(info.image.clone()),
                uv_transform: uv(&leg, width, now),
                ..default()
            },
            Layer::Glow.depth_bias(),
        ));
        let direction = Vec2::from_angle(leg.angle);
        let middle = leg.start + direction * leg.length() / 2.0;
        commands.spawn((
            Name::new("Path line"),
            MapEntity,
            DottedLeg { owner, leg, sprite_width: width, shown: now },
            Mesh3d(quad.0.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(middle.extend(z))
                .with_rotation(Quat::from_rotation_z(leg.angle))
                .with_scale(Vec3::new(leg.length(), d.width / PX_PER_UNIT, 1.0)),
        ));
    }
    true
}

/// `Path.Destroy`: the line goes when its entity does.
fn drop_orphans(mut commands: Commands, legs: Query<(Entity, &DottedLeg)>, owners: Query<(), With<MapId>>) {
    for (e, leg) in &legs {
        if !owners.contains(leg.owner) {
            commands.entity(e).despawn();
        }
    }
}

/// The dots march.
fn march(
    time: Res<Time>,
    mut legs: Query<(&mut DottedLeg, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = offset(time.elapsed_secs());
    for (mut leg, material) in &mut legs {
        if leg.shown == now {
            continue;
        }
        leg.shown = now;
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.uv_transform = uv(&leg.leg, leg.sprite_width, now);
        }
    }
}
