//! Shadows of cars and loose objects: sprites, not shadow maps.
//!
//! C# origin: `Entities/Components/DynamicShadow.cs` (2012–2014), added to
//! every car and physical entity whose shadow is not baked into the light.
//! The baked light already has the walls' and fixed objects' shadows;
//! moving things get a cheap fake that reads the same:
//!
//! - **The streak** (`Gradient4Pixel`): a soft black band from the object
//!   away from its spot's lamp ([`ShadowLamps`]). Darkest halfway out to
//!   the lamp's radius, none at the lamp or at the radius. It reaches up
//!   to four object radii, less near the radius's end and never much
//!   further than the nearest wall. It is as wide as the object seen from
//!   the lamp: its width side-on, its length end-on, its diagonal between.
//! - **Penumbra**: when the object looks wider than 2° from the lamp, the
//!   streak is two, turned apart by that half-angle, at half strength each;
//!   from 20° to 45° they fade out (right under a lamp there is no shadow).
//! - **Hand-over**: crossing into another lamp's spots, the old shadow
//!   fades out while the new one fades in, over half a second.
//! - **Contact shadow** (`Gradient4PixelUniform`): a quarter-black soft
//!   box, 1.1 × twice the object's size, under it whatever the lamps.
//!
//! The sizes are C#'s, in world units (its sim units). The streak's length
//! is the object's width/3 per unit of reach: bigger objects throw longer
//! shadows.

use bevy::prelude::*;

use crate::game::{GameState, LoadedMap, MapEntity};
use crate::light::shadow_lamps::ShadowLamps;
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

/// `Settings.DynamicComplexShadowTreshold`: wider than this, two streaks.
const SPLIT: f32 = 2.0 * std::f32::consts::PI / 180.0;
/// `DynamicComplexShadowAlhpaAngleStart` / `End`: the pair fades out.
const FADE_START: f32 = 20.0 * std::f32::consts::PI / 180.0;
const FADE_END: f32 = 45.0 * std::f32::consts::PI / 180.0;
/// Hand-over per second (`UpdateTimeSec * 2` a frame at 60 fps).
const HANDOVER_RATE: f32 = 2.0;
/// `_shadowTextureBlackProportion`: the gradient is black over 2 of its 4
/// pixels, so it is drawn 1.75 times as wide as the shadow.
const BLACK_PROPORTION: f32 = 1.75;
/// The contact shadow's opacity (`Color.Black * 0.25f`).
const CONTACT_ALPHA: f32 = 0.25;

/// Casts a shadow sprite; `size` is its body (length, width) in world
/// units, as its sprite.
#[derive(Component, Debug, Clone, Copy)]
pub struct CastsShadow {
    pub size: Vec2,
}

/// Where each lamp's shadow falls, built with the map.
#[derive(Resource)]
pub struct ShadowMap {
    lamps: ShadowLamps,
    /// `MapManager.DistanceField`, in cells.
    distance: Vec<f32>,
}

/// The shadow of one caster, which it follows without turning.
#[derive(Component, Debug)]
struct Shadow {
    caster: Entity,
    from: Option<usize>,
    to: Option<usize>,
    /// 0 → 1 from `from` to `to`; 1 when there is no hand-over.
    handover: f32,
}

/// A streak sprite: `pair` 0 for the lamp handed over from, 1 for the
/// current one; `half` 0 or 1 of a penumbra pair.
#[derive(Component, Debug)]
struct Streak {
    pair: usize,
    half: usize,
}

#[derive(Component, Debug)]
struct Contact;

pub struct ShadowPlugin;

impl Plugin for ShadowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build_shadow_map).add_systems(
            PostUpdate,
            (spawn_shadows, update_shadows)
                .chain()
                .before(TransformSystems::Propagate)
                .run_if(in_state(GameState::Playing))
                .run_if(resource_exists::<ShadowMap>),
        );
    }
}

fn build_shadow_map(mut commands: Commands, map: Res<LoadedMap>) {
    commands.insert_resource(ShadowMap {
        lamps: ShadowLamps::build(&map.grid, &map.light.frame, &map.light.lamps),
        distance: map.grid.chamfer_distance(),
    });
}

fn spawn_shadows(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    casters: Query<(Entity, &CastsShadow), Added<CastsShadow>>,
) {
    let (Some(streak), Some(contact)) = (map.sprites.get("Gradient4Pixel"), map.sprites.get("Gradient4PixelUniform"))
    else {
        return;
    };
    let tint = SpriteTint {
        color: [0.0, 0.0, 0.0, 0.0],
        lit: false,
    };
    for (caster, _) in &casters {
        commands
            .spawn((
                Name::new("Shadow"),
                MapEntity,
                Shadow {
                    caster,
                    from: None,
                    to: None,
                    handover: 1.0,
                },
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|p| {
                for pair in 0..2 {
                    for half in 0..2 {
                        p.spawn((
                            sprite_bundle(streak, 1.0, Layer::Shadow, tint, &quad, &mut materials),
                            Streak { pair, half },
                            Visibility::Hidden,
                        ));
                    }
                }
                let tint = SpriteTint {
                    color: [0.0, 0.0, 0.0, darkening(CONTACT_ALPHA)],
                    lit: false,
                };
                p.spawn((sprite_bundle(contact, 1.0, Layer::Shadow, tint, &quad, &mut materials), Contact));
            });
    }
}

/// One lamp's streak, around the caster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreakShape {
    /// World angle away from the lamp.
    pub dir: f32,
    /// Half the angle between a penumbra pair; 0 for a single streak.
    pub split: f32,
    /// Size of one texel of the 4×4 gradient, world units: along the
    /// streak and across it.
    pub texel: Vec2,
    /// Opacity of each streak.
    pub alpha: f32,
}

/// What a lamp makes of a caster's shadow (`DynamicShadow.UpdateCache`,
/// `UpdateCacheForLight`), all in world units: the caster at `pos`
/// turned by `angle`, `size` (length, width); the lamp at `lamp` with
/// radius `radius`; `wall` the distance field at the caster plus a cell.
pub fn streak_shape(pos: Vec2, angle: f32, size: Vec2, lamp: Vec2, radius: f32, wall: f32) -> Option<StreakShape> {
    let v = pos - lamp;
    let l = v.length();
    let amount = 1.0 - (l / radius - 0.5).abs() * 2.0;
    let reach = (radius - l).min(size.max_element() / 2.0 * 4.0).min(wall);
    if reach <= 0.0 || l <= 0.0 {
        return None;
    }
    let dir = v.y.atan2(v.x);
    // How wide the caster looks from the lamp, in widths.
    let (w, h) = (size.x, size.y);
    let corner = h.atan2(w);
    let diagonal = ((w / h).powi(2) + 1.0).sqrt();
    let a = (dir - angle).abs();
    let side = if (a / std::f32::consts::FRAC_PI_2) as i32 % 2 == 0 {
        (a + corner).sin().abs()
    } else {
        (a - corner).sin().abs()
    };
    let across = side * diagonal * 0.95;
    let half_angle = h * across / l / 2.0;
    let mut alpha = amount;
    let mut split = 0.0;
    if half_angle > SPLIT {
        split = half_angle;
        alpha *= 0.5;
        if half_angle > FADE_START {
            alpha *= (FADE_END - half_angle) / (FADE_END - FADE_START);
        }
    }
    (alpha > 0.0).then_some(StreakShape {
        dir,
        split,
        // `_baseScale`: the width over 4 texels × 4/3 (the origin is a
        // quarter in), times the reach; across, × 1.75 for the black part.
        texel: Vec2::new(h / 3.0 * reach, h / 4.0 * BLACK_PROPORTION * across),
        alpha,
    })
}

/// XNA blended in sRGB, Bevy blends in linear light, where the same black
/// alpha darkens less; this alpha darkens in linear light as `a` did.
fn darkening(a: f32) -> f32 {
    1.0 - (1.0 - a.clamp(0.0, 1.0)).powf(2.2)
}

type StreakItem<'a> = (&'a Streak, &'a mut Transform, &'a mut SpriteTint, &'a mut Visibility);

#[allow(clippy::type_complexity)]
fn update_shadows(
    time: Res<Time>,
    (map, shadow_map): (Res<LoadedMap>, Res<ShadowMap>),
    casters: Query<(&Transform, &CastsShadow), Without<Shadow>>,
    mut shadows: Query<(Entity, &mut Shadow, &mut Transform, &Children), Without<CastsShadow>>,
    mut streaks: Query<StreakItem, (Without<Shadow>, Without<CastsShadow>, Without<Contact>)>,
    mut contacts: Query<&mut Transform, (With<Contact>, Without<Streak>, Without<Shadow>, Without<CastsShadow>)>,
    mut commands: Commands,
) {
    let cell = map.cell.0;
    for (entity, mut shadow, mut root, children) in &mut shadows {
        let Ok((caster, casts)) = casters.get(shadow.caster) else {
            // The caster is gone (picked up, destroyed).
            commands.entity(entity).despawn();
            continue;
        };
        let pos = caster.translation.truncate();
        let angle = caster.rotation.to_euler(EulerRot::XYZ).2;
        root.translation = pos.extend(0.0);

        let (cx, cy) = map.cell.to_cells(pos);
        let lamp = shadow_map.lamps.lamp_at(cx, cy, cell);
        if lamp != shadow.to {
            shadow.from = shadow.to;
            shadow.to = lamp;
            shadow.handover = 1.0 - shadow.handover;
        }
        let pairs = if shadow.from == shadow.to {
            [(None, 0.0), (shadow.to, 1.0)]
        } else {
            [(shadow.from, 1.0 - shadow.handover), (shadow.to, shadow.handover)]
        };
        if shadow.from != shadow.to {
            shadow.handover += time.delta_secs() * HANDOVER_RATE;
            if shadow.handover >= 1.0 {
                shadow.handover = 1.0;
                shadow.from = shadow.to;
            }
        }
        let wall = wall_distance(&map, &shadow_map.distance, cx, cy) + cell;
        let shapes = pairs.map(|(lamp, weight)| {
            let lamp = &map.light.lamps[lamp?];
            let at = map.world(lamp.x, lamp.y);
            streak_shape(pos, angle, casts.size, at, lamp.radius * cell, wall).map(|s| (s, weight))
        });

        for child in children.iter() {
            if let Ok(mut t) = contacts.get_mut(child) {
                t.rotation = Quat::from_rotation_z(angle);
                t.scale = (casts.size * 2.0 * 1.1).extend(1.0);
                continue;
            }
            let Ok((streak, mut t, mut tint, mut visibility)) = streaks.get_mut(child) else {
                continue;
            };
            let Some((shape, weight)) = shapes[streak.pair] else {
                *visibility = Visibility::Hidden;
                continue;
            };
            let dir = match (shape.split > 0.0, streak.half) {
                (false, 1) => {
                    *visibility = Visibility::Hidden;
                    continue;
                }
                (false, _) => shape.dir,
                (true, 0) => shape.dir + shape.split,
                (true, _) => shape.dir - shape.split,
            };
            *visibility = Visibility::Inherited;
            tint.color[3] = darkening(shape.alpha * weight);
            // The texture's origin is its texel (1, 2): the quad's centre
            // is a texel ahead of the caster.
            t.translation = (Vec2::from_angle(dir) * shape.texel.x).extend(Layer::Shadow.z());
            t.rotation = Quat::from_rotation_z(dir);
            t.scale = (shape.texel * 4.0).extend(1.0);
        }
    }
}

/// `MapManager.GetDistanceFieldValue`, world units: 0 on the map's outer
/// two rings and outside it.
fn wall_distance(map: &LoadedMap, distance: &[f32], x: f32, y: f32) -> f32 {
    let (x, y) = (x as i32, y as i32);
    let (w, h) = (map.grid.width as i32, map.grid.height as i32);
    if x <= 0 || y <= 0 || x >= w - 2 || y >= h - 2 {
        return 0.0;
    }
    distance[(y * w + x) as usize] * map.cell.0
}
