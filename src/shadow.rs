//! The car's shadow: sprites, not shadow maps (`DynamicShadow.cs`).
//!
//! - A soft streak (`Gradient4Pixel`) stretches away from the nearest lamp
//!   that casts dynamic shadows and can see the car. It is darkest at half
//!   the lamp's radius, longer further from the lamp (up to four car
//!   radii, and not past the nearest wall), and wider when the car is side-on
//!   to the light.
//! - When the lamp is close, the streak splits in two, turned apart by the
//!   car's angular size seen from the lamp, at half strength: a penumbra.
//!   Closer still (20°–45°) it fades out.
//! - Driving from one lamp's light into another's, the old shadow fades out
//!   and the new one in over half a second.
//! - A faint blob (`Gradient4PixelUniform`, 25% black) always sits under
//!   the car.

use bevy::prelude::*;

use crate::car::Car;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::light::bake::Lamp;
use crate::render::sprite::{SpriteInfo, SpriteTint, UnitQuad, sprite_bundle};
use crate::units::px;

const Z_SHADOW: f32 = 0.01;
const Z_AMBIENT: f32 = 0.012;
/// `Settings.DynamicComplexShadowTreshold` and the fade angles.
const SPLIT_ANGLE: f32 = 2.0 * std::f32::consts::PI / 180.0;
const FADE_START: f32 = 20.0 * std::f32::consts::PI / 180.0;
const FADE_END: f32 = 45.0 * std::f32::consts::PI / 180.0;
/// Seconds to hand the shadow over from one lamp to another.
const HANDOVER: f32 = 0.5;
/// The gradient's opaque part is 2 of its 4 pixels.
const BLACK_PROPORTION: f32 = 1.75;

/// The shadow state of a car; its sprites are children of the shadow root.
#[derive(Component, Debug, Default)]
pub struct CarShadow {
    from: Option<usize>,
    to: Option<usize>,
    /// 0..1 progress from `from` to `to`.
    handover: f32,
}

/// One of the four streak sprites: (lamp slot 0 = from / 1 = to, half 0/1).
#[derive(Component, Debug)]
struct Streak {
    slot: usize,
    half: usize,
}

pub struct ShadowPlugin;

impl Plugin for ShadowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), add_car_shadows.after(crate::car::spawn_player_car))
            .add_systems(PostUpdate, update_shadows.before(TransformSystems::Propagate).run_if(in_state(GameState::Playing)));
    }
}

fn add_car_shadows(
    mut commands: Commands,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cars: Query<Entity, With<Car>>,
) {
    let (Some(streak), Some(blob)) = (map.sprites.get("Gradient4Pixel"), map.sprites.get("Gradient4PixelUniform"))
    else {
        warn!("shadow sprites missing");
        return;
    };
    for car in &cars {
        // The shadow lives beside the car (not as a child) so it does not turn with it.
        let mut root = commands.spawn((
            Name::new("Car shadow"),
            MapEntity,
            CarShadow::default(),
            ShadowOf(car),
            Transform::default(),
            Visibility::default(),
        ));
        root.with_children(|p| {
            for slot in 0..2 {
                for half in 0..2 {
                    p.spawn((streak_bundle(streak, &quad, &mut materials), Streak { slot, half }));
                }
            }
            let tint = SpriteTint {
                color: [0.0, 0.0, 0.0, 0.25],
                lit: false,
            };
            p.spawn((sprite_bundle(blob, 1.0, Z_AMBIENT, tint, &quad, &mut materials), AmbientBlob));
        });
    }
}

#[derive(Component, Debug)]
struct ShadowOf(Entity);

#[derive(Component, Debug)]
struct AmbientBlob;

fn streak_bundle(info: &SpriteInfo, quad: &UnitQuad, materials: &mut Assets<StandardMaterial>) -> impl Bundle {
    let tint = SpriteTint {
        color: [0.0, 0.0, 0.0, 0.0],
        lit: false,
    };
    sprite_bundle(info, 1.0, Z_SHADOW, tint, quad, materials)
}

/// Geometry of one streak for one lamp.
struct StreakShape {
    dir: f32,
    split: f32,
    length: f32,
    width: f32,
    alpha: f32,
}

fn streak_for(map: &LoadedMap, car_pos: Vec2, car_angle: f32, car_size: Vec2, lamp: &Lamp) -> Option<StreakShape> {
    let lamp_pos = map.world(lamp.x, lamp.y);
    let radius = lamp.radius * map.cell.0;
    let v = car_pos - lamp_pos;
    let l = v.length();
    let amount = 1.0 - ((l / radius) - 0.5).abs() * 2.0;
    let (cx, cy) = map.cell.to_cells(car_pos);
    let wall_room = map.grid.distance_to_wall(cx.floor() as i32, cy.floor() as i32) * map.cell.0 + map.cell.0;
    let car_radius = car_size.max_element() / 2.0;
    let length = (radius - l).min(car_radius * 4.0).min(wall_room);
    if length <= 0.0 || amount <= 0.0 {
        return None;
    }
    let dir = v.y.atan2(v.x);
    // Width: how much of the car the light sees side-on.
    let (w, h) = (car_size.x, car_size.y);
    let angle_delta = h.atan2(w);
    let diagonal = ((w / h).powi(2) + 1.0).sqrt();
    let angle = (dir - car_angle).abs();
    let side = if ((angle / std::f32::consts::FRAC_PI_2) as i32) % 2 == 0 {
        (angle + angle_delta).sin().abs()
    } else {
        (angle - angle_delta).sin().abs()
    };
    let width = h * BLACK_PROPORTION * side * diagonal * 0.95;
    let split = (width / BLACK_PROPORTION / l.max(0.01)) / 2.0;
    let mut alpha = amount;
    let mut split_angle = 0.0;
    if split > SPLIT_ANGLE {
        split_angle = split;
        alpha *= 0.5;
        if split > FADE_START {
            alpha *= ((FADE_END - split) / (FADE_END - FADE_START)).max(0.0);
        }
    }
    Some(StreakShape {
        dir,
        split: split_angle,
        length: h * 4.0 / 3.0 * length,
        width,
        alpha,
    })
}

#[allow(clippy::type_complexity)]
fn update_shadows(
    time: Res<Time>,
    map: Res<LoadedMap>,
    cars: Query<(&Transform, &Collider), With<Car>>,
    mut shadows: Query<(&mut CarShadow, &ShadowOf, &mut Transform, &Children), Without<Car>>,
    mut streaks: Query<(&Streak, &mut Transform, &mut SpriteTint), (Without<CarShadow>, Without<Car>, Without<AmbientBlob>)>,
    mut blobs: Query<&mut Transform, (With<AmbientBlob>, Without<Streak>, Without<CarShadow>, Without<Car>)>,
) {
    let dt = time.delta_secs();
    for (mut shadow, of, mut root, children) in &mut shadows {
        let Ok((car_t, collider)) = cars.get(of.0) else {
            continue;
        };
        let car_pos = car_t.translation.truncate();
        let car_angle = car_t.rotation.to_euler(EulerRot::XYZ).2;
        let car_size = collider_size(collider);
        root.translation = car_pos.extend(0.0);

        // The nearest lamp that casts dynamic shadows and sees the car.
        let (cx, cy) = map.cell.to_cells(car_pos);
        let lamp = map
            .light
            .lamps
            .iter()
            .enumerate()
            .filter(|(_, l)| l.use_for_dynamic_shadow)
            .filter(|(_, l)| {
                let d2 = (l.x - cx).powi(2) + (l.y - cy).powi(2);
                d2 < l.radius * l.radius && d2 * map.cell.0 * map.cell.0 > 0.75 * 0.75
                    && !map.grid.line_hits_wall((l.x, l.y), (cx, cy))
            })
            .min_by(|(_, a), (_, b)| {
                let da = (a.x - cx).powi(2) + (a.y - cy).powi(2);
                let db = (b.x - cx).powi(2) + (b.y - cy).powi(2);
                da.total_cmp(&db)
            })
            .map(|(i, _)| i);
        if lamp != shadow.to {
            shadow.from = shadow.to;
            shadow.to = lamp;
            shadow.handover = 1.0 - shadow.handover;
        }
        if shadow.from == shadow.to {
            shadow.handover = 1.0;
        } else {
            shadow.handover = (shadow.handover + dt / HANDOVER).min(1.0);
            if shadow.handover >= 1.0 {
                shadow.from = shadow.to;
            }
        }

        let slots = [(shadow.from, 1.0 - shadow.handover), (shadow.to, shadow.handover)];
        for child in children.iter() {
            if let Ok(mut blob) = blobs.get_mut(child) {
                blob.rotation = Quat::from_rotation_z(car_angle);
                blob.scale = (car_size * 2.0 * 1.1).extend(1.0) / 2.0;
                continue;
            }
            let Ok((streak, mut t, mut tint)) = streaks.get_mut(child) else {
                continue;
            };
            let (lamp, weight) = slots[streak.slot];
            let weight = if shadow.from == shadow.to && streak.slot == 0 { 0.0 } else { weight };
            let shape = lamp.and_then(|i| streak_for(&map, car_pos, car_angle, car_size, &map.light.lamps[i]));
            let Some(shape) = shape.filter(|_| weight > 0.0) else {
                tint.color[3] = 0.0;
                continue;
            };
            let dir = match (shape.split > 0.0, streak.half) {
                (false, 1) => {
                    tint.color[3] = 0.0;
                    continue;
                }
                (false, _) => shape.dir,
                (true, 0) => shape.dir + shape.split,
                (true, _) => shape.dir - shape.split,
            };
            tint.color[3] = shape.alpha * weight;
            // The texture's origin is its pixel (1, 2): a quarter in from the back.
            let forward = Vec2::from_angle(dir);
            t.translation = (forward * shape.length * 0.25).extend(Z_SHADOW);
            t.rotation = Quat::from_rotation_z(dir);
            t.scale = Vec3::new(shape.length, shape.width, 1.0);
        }
    }
}

fn collider_size(collider: &avian2d::prelude::Collider) -> Vec2 {
    collider
        .shape_scaled()
        .as_cuboid()
        .map(|c| Vec2::new(c.half_extents.x, c.half_extents.y) * 2.0)
        .unwrap_or(Vec2::splat(px(64.0)))
}

use avian2d::prelude::Collider;
