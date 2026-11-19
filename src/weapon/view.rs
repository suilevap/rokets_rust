//! What the gun looks like: its sprite on the cabin, the aim line
//! (`LaserRenderer`, `LaserView`) and the energy bar
//! (`EnergyComponent.DrawOnUI`).
//!
//! The aim line runs from the muzzle to the first thing a bullet would
//! hit, up to the gun's range, and on for a gun that bounces its bullets
//! (the Railgun) along the bounce, as `LaserRenderer`'s `ReflectionCount`
//! did (as far as the range is left; not off what can be hurt, or at a
//! head-on blow). Normally it is a thin green line (3 C#
//! pixels) of a noise strip (`Special\LaserLine`) whose rows scroll once
//! a second, so it shimmers, with a wider faint glow (`LaserLine2`, the
//! `ImprovedLaserView` the desktop had). While the gun cannot fire (out
//! of energy, or between pulls), it is white, pulsing once a second and
//! up to 8 pixels wide: solid when it ends on something, fading out at
//! its end otherwise. A gun that fires only accurately shows its reload
//! the same way, white, as wide as 26 pixels at the start of it and
//! narrowing to 3 as it runs out, brightening (`SetLaserReload`).
//! Drawing it asked the energy whether it could fire at all
//! (`CheckEnergy`), which reset the bar's last-asked mark; so does this.
//!
//! The energy bar sits under the top edge, 144 × 4 pixels at 800 × 480
//! (scaled with the screen as `UISizeMultiplier`): a faint track, the
//! energy in white, the energy just spent trailing behind in red
//! (flashing red and yellow while it is short of what was asked), and a
//! jolt when it is spent, runs dry or fills. Full, it fades out over
//! 3 s.
//!
//! Not ported: the laser's sparkles on the
//! player's own car (`CreateLaserEffect`); the extra offset C# gave the bar in hot-seat games
//! (`PlayerUIOffset`).

use avian2d::prelude::*;
use bevy::math::Affine2;
use bevy::prelude::*;
use rand::Rng;

use super::model::{self, AimLook, Energy, MAX_ENERGY, SHOW_FULL};
use super::{BULLET_REACH, Hurt, PassedByBullets, PlayerEnergy, Weapon, WeaponSystems, sprite_of};
use crate::controls::{LocalPlayer, PlayerIntent};
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::units::PX_PER_UNIT;

const LASER_LINE: &str = "Special\\LaserLine";
const LASER_GLOW: &str = "Special\\LaserLine2";
const SOLID: &str = "WhitePixel";
const FADING: &str = "Gradient8WidthPixelNonSym";

/// Textures the aim line draws with, to load with a map that has a gun.
pub const AIM_LINE_TEXTURES: [&str; 4] = [LASER_LINE, LASER_GLOW, SOLID, FADING];

/// `LaserView.MaxLength`: the strip's full length, world units.
const STRIP_LENGTH: f32 = 32.0;

/// The gun's sprite on a car.
#[derive(Component)]
struct GunView;

/// The most legs the aim line has: the muzzle's and bounces (a gun
/// bounces its bullets once; more would be cut short).
const MAX_LEGS: usize = 4;

/// One leg of the aim line: its line or its glow.
#[derive(Component)]
struct AimLine {
    glow: bool,
    leg: usize,
}

/// A leg of the aim line.
#[derive(Clone, Copy)]
struct Leg {
    start: Vec2,
    end: Vec2,
    /// It ends on something (not just at the range).
    hit: bool,
}

/// The energy bar's parts.
#[derive(Component)]
enum BarPart {
    Track,
    Trail,
    Fill,
}

#[derive(Component)]
struct EnergyBar;

/// Where the fire zone is, shown on touch screens (as the gadget's).
#[derive(Component)]
struct FireZone;

const ZONE_CIRCLE: f32 = 96.0;

pub struct WeaponViewPlugin;

impl Plugin for WeaponViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_energy_bar, spawn_fire_zone))
            .add_systems(Update, show_fire_zone)
            .add_systems(
            Update,
            (spawn_gun_views, place_gun_views, draw_aim_lines, draw_energy_bar)
                .chain()
                .after(WeaponSystems)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// The gun's sprite, turned about its origin, as a child of the car.
fn spawn_gun_views(
    mut commands: Commands,
    guns: Query<(Entity, &Weapon, Option<&Children>)>,
    views: Query<(), With<GunView>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (car, gun, children) in &guns {
        if children.is_some_and(|c| c.iter().any(|e| views.contains(e))) {
            continue;
        }
        let def = sprite_of(gun.kind);
        let Some(info) = map.sprites.get(def.name) else {
            continue;
        };
        let lit = SpriteTint {
            color: [1.0; 4],
            lit: true,
        };
        let mut sprite = sprite_bundle(info, 1.0, Layer::Weapon, lit, &quad, &mut materials);
        // From the turning point to the sprite's middle (texture y down).
        let centre = def.size / 2.0 - def.origin;
        sprite.2.translation.x = centre.x / PX_PER_UNIT;
        sprite.2.translation.y = -centre.y / PX_PER_UNIT;
        let pivot = commands
            .spawn((Name::new("Gun"), GunView, Transform::default(), Visibility::default()))
            .with_child(sprite)
            .id();
        commands.entity(car).add_child(pivot);
    }
}

fn place_gun_views(guns: Query<(&Weapon, &Children)>, mut views: Query<&mut Transform, With<GunView>>) {
    for (gun, children) in &guns {
        for child in children.iter() {
            if let Ok(mut t) = views.get_mut(child) {
                t.translation = gun.local.extend(0.0);
                t.rotation = Quat::from_rotation_z(gun.local_angle);
            }
        }
    }
}

/// `PhaseHelper.Triangle_Period500`: 0 → 1 → 0 once a second (its name
/// notwithstanding).
fn triangle(seconds: f32) -> f32 {
    let r = (seconds % 1.0) * 2.0;
    if r > 1.0 { 2.0 - r } else { r }
}

/// `LaserRenderer.Render`'s legs: from the muzzle, then bounced `reflections`
/// times while the range lasts, off what cannot be hurt and not at a
/// head-on blow.
fn aim_legs(
    gun: &Weapon,
    car: Entity,
    spatial: &SpatialQuery,
    skipped: &Query<(), PassedByBullets>,
    hurt: &Query<(), Hurt>,
) -> Vec<Leg> {
    let filter = SpatialQueryFilter::from_excluded_entities([car]);
    let mut legs = Vec::new();
    let (mut cast_from, mut start, mut direction, mut length) = (gun.at, gun.muzzle(), gun.direction, gun.options.range);
    let bounces = (gun.options.reflections as usize).min(MAX_LEGS - 1);
    for i in 0..=bounces {
        let dir = Vec2::from_angle(direction);
        let hit = Dir2::new(dir).ok().and_then(|d| {
            spatial.cast_ray_predicate(cast_from, d, length.min(BULLET_REACH), true, &filter, &|e| !skipped.contains(e))
        });
        let end = cast_from + dir * hit.map_or(length, |h| h.distance);
        legs.push(Leg {
            start,
            end,
            hit: hit.is_some(),
        });
        length -= start.distance(end);
        let Some(hit) = hit.filter(|_| i < bounces && length > 0.0) else {
            break;
        };
        // Not off what can be hurt, nor with no surface to bounce off.
        if hurt.contains(hit.entity) {
            break;
        }
        let Some(bounced) = model::reflect(direction, hit.normal) else {
            break;
        };
        direction = bounced;
        cast_from = end + hit.normal * super::BOUNCE_OFFSET;
        start = end;
    }
    legs
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_aim_lines(
    mut commands: Commands,
    time: Res<Time>,
    flash: Res<crate::render::particles::FlashBoost>,
    spatial: SpatialQuery,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut guns: Query<(Entity, &Weapon, &PlayerIntent, Option<&mut PlayerEnergy>), With<LocalPlayer>>,
    skipped: Query<(), PassedByBullets>,
    hurt: Query<(), Hurt>,
    mut lines: Query<(&AimLine, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
) {
    if lines.is_empty() {
        for leg in 0..MAX_LEGS {
            for glow in [false, true] {
                let material = materials.add(StandardMaterial {
                    base_color_texture: map.sprites.get(LASER_LINE).map(|s| s.image.clone()),
                    alpha_mode: AlphaMode::Add,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    depth_bias: Layer::Glow.depth_bias(),
                    ..default()
                });
                commands.spawn((
                    Name::new("Aim line"),
                    MapEntity,
                    AimLine { glow, leg },
                    Mesh3d(quad.0.clone()),
                    MeshMaterial3d(material),
                    Transform::default(),
                    Visibility::Hidden,
                ));
            }
        }
        return;
    }
    let shown = guns.iter_mut().next().and_then(|(car, gun, intent, energy)| {
        // `LaserVisiblityUpdate`: while aiming, not while the gadget charges.
        if !gun.aiming || intent.gadget {
            return None;
        }
        // `UpdateLaser`: low while it cannot fire, wide while it reloads.
        let look = gun.aim_look(energy.map(|e| &mut e.into_inner().0));
        Some((aim_legs(gun, car, &spatial, &skipped, &hurt), look))
    });
    let t = time.elapsed_secs();
    for (line, mut transform, mut visibility, material) in &mut lines {
        let Some(leg) = shown.as_ref().and_then(|(legs, look)| legs.get(line.leg).map(|l| (*l, *look))) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let (Leg { start, end, hit }, look) = leg;
        let length = start.distance(end);
        // The glow goes with the green line only.
        if length <= 0.0 || (look != AimLook::Normal && line.glow) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Some(mut m) = materials.get_mut(&material.0) else {
            continue;
        };
        let white = |alpha: f32| Color::srgba(1.0, 1.0, 1.0, alpha);
        let (width, color) = match look {
            AimLook::Normal => (3.0, Color::srgba(0.0, 128.0 / 255.0 * if line.glow { 0.5 } else { 1.0 }, 0.0, 1.0)),
            AimLook::Low => {
                let tri = triangle(t);
                (3.0 + 5.0 * (1.0 - tri), white(0.1 + 0.9 * tri))
            }
            AimLook::Reload(left) => (3.0 + 23.0 * left, white(1.0 - left * 0.5)),
        };
        let (texture, width, uv) = if width <= 3.0 {
            // A row of the strip, as long a part of it as the line is of 32
            // (`Color.Green` is (0, 128, 0); the glow at half and wider).
            let rows = if line.glow { 33.0 } else { 16.0 };
            let row = ((t % 1.0) * 15.0).floor();
            let row = if line.glow { row * 2.0 + 1.0 } else { row };
            let part = ((length / STRIP_LENGTH).min(1.0) * 128.0).floor().max(1.0) / 128.0;
            let uv = Affine2::from_scale_angle_translation(Vec2::new(part, 1.0 / rows), 0.0, Vec2::new(0.0, row / rows));
            let w = if line.glow { 12.0 } else { width };
            (if line.glow { LASER_GLOW } else { LASER_LINE }, w, uv)
        } else if hit || length > STRIP_LENGTH {
            (SOLID, width, Affine2::IDENTITY)
        } else {
            // The fading strip runs from its solid end at the leg's start.
            let uv = Affine2::from_scale_angle_translation(Vec2::new(-1.0, 1.0), 0.0, Vec2::new(1.0, 0.0));
            (FADING, width, uv)
        };
        let image = map.sprites.get(texture).map(|s| s.image.clone());
        if m.base_color_texture != image {
            m.base_color_texture = image;
        }
        // Additive, as the flashes: the same boost for linear light.
        let c = color.to_linear();
        m.base_color = Color::linear_rgba(c.red * flash.0, c.green * flash.0, c.blue * flash.0, c.alpha);
        m.uv_transform = uv;
        let mid = (start + end) / 2.0;
        *transform = Transform::from_translation(mid.extend(Layer::Glow.z()))
            .with_rotation(Quat::from_rotation_z((end - start).to_angle()))
            .with_scale(Vec3::new(length, width / PX_PER_UNIT, 1.0));
        *visibility = Visibility::Inherited;
    }
}

/// C#'s screens were 800 × 480; its UI grew with the screen.
fn ui_scale(window: &Window) -> f32 {
    (window.width() / 800.0).min(window.height() / 480.0)
}

fn spawn_energy_bar(mut commands: Commands) {
    commands
        .spawn((
            EnergyBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_children(|bar| {
            for part in [BarPart::Track, BarPart::Trail, BarPart::Fill] {
                bar.spawn((
                    part,
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Pickable::IGNORE,
                ));
            }
        });
}

/// How visible the bar is: fully while not full, then fading over 3 s.
pub fn energy_bar_alpha(e: &Energy) -> f32 {
    if e.current >= MAX_ENERGY { (e.full_shown / SHOW_FULL).max(0.0) } else { 1.0 }
}

#[allow(clippy::type_complexity)]
fn draw_energy_bar(
    time: Res<Time>,
    mut rng: ResMut<super::WeaponRng>,
    windows: Query<&Window>,
    mut energies: Query<&mut PlayerEnergy, With<LocalPlayer>>,
    mut bars: Query<(&mut Node, &mut Visibility), (With<EnergyBar>, Without<BarPart>)>,
    mut parts: Query<(&BarPart, &mut Node, &mut BackgroundColor), Without<EnergyBar>>,
) {
    let Ok((mut bar, mut visibility)) = bars.single_mut() else {
        return;
    };
    let energy = energies.iter_mut().next();
    let alpha = energy.as_ref().map_or(0.0, |e| energy_bar_alpha(&e.0));
    if alpha <= 0.0 {
        *visibility = Visibility::Hidden;
        return;
    }
    let Some(mut energy) = energy else {
        return;
    };
    *visibility = Visibility::Inherited;
    let k = windows.iter().next().map_or(1.0, ui_scale);
    let width = 24.0 * 6.0 * k;
    let e = &mut energy.0;
    // The jolt: up and down by up to its amount, easing off.
    let mut jolt = 0.0;
    if e.shake.current > 0.0 {
        e.shake.destination = 0.0;
        e.shake.step(time.delta_secs());
        jolt = e.shake.current * (rng.0.random::<f32>() * 2.0 - 1.0) * e.shake_amount * k;
    }
    bar.top = Val::Px(16.0 * 2.0 * k + jolt);
    bar.margin.left = Val::Px(-width / 2.0);
    let flashing = e.current < e.last_check;
    let t = time.elapsed_secs();
    for (part, mut node, mut color) in &mut parts {
        let (share, c) = match part {
            BarPart::Track => (1.0, Color::srgba(1.0, 1.0, 1.0, 0.25 * alpha)),
            BarPart::Trail => {
                let red = Color::srgba(1.0, 0.0, 0.0, 0.8 * alpha);
                let c = if flashing {
                    // Red to yellow ten times a second (`Sawtooth_Period100`).
                    let s = (t % 0.1) / 0.1;
                    Color::srgba(1.0, s, 0.0, alpha)
                } else {
                    red
                };
                (if e.trail.current > e.current { e.trail.current } else { 0.0 }, c)
            }
            BarPart::Fill => (e.current, Color::srgba(1.0, 1.0, 1.0, alpha)),
        };
        node.width = Val::Px(width * share);
        node.height = Val::Px(4.0 * k);
        color.0 = c;
    }
}

fn spawn_fire_zone(mut commands: Commands) {
    commands.spawn((
        FireZone,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(75.0),
            top: Val::Percent(75.0),
            margin: UiRect {
                left: Val::Px(-ZONE_CIRCLE / 2.0),
                top: Val::Px(-ZONE_CIRCLE / 2.0),
                ..default()
            },
            width: Val::Px(ZONE_CIRCLE),
            height: Val::Px(ZONE_CIRCLE),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
        Pickable::IGNORE,
    ));
}

/// Faint while it waits, bright while a finger holds it; in the middle of
/// the zone (`controls::in_fire_zone`): the bottom right quarter beside a
/// gadget, the right half without one.
#[allow(clippy::type_complexity)]
fn show_fire_zone(
    scheme: Res<crate::controls::ActiveScheme>,
    touch: Res<crate::controls::FireTouch>,
    guns: Query<Has<crate::gadget::Magnet>, (With<Weapon>, With<LocalPlayer>)>,
    mut zones: Query<(&mut Node, &mut BorderColor, &mut BackgroundColor), With<FireZone>>,
) {
    let gun = guns.iter().next();
    let shown = *scheme == crate::controls::ActiveScheme::Touch && gun.is_some();
    let alpha = if !shown { 0.0 } else if touch.finger.is_some() { 0.6 } else { 0.25 };
    let top = Val::Percent(if gun == Some(true) { 75.0 } else { 50.0 });
    for (mut node, mut border, mut back) in &mut zones {
        if node.top != top {
            node.top = top;
        }
        let edge = Color::srgba(1.0, 1.0, 1.0, alpha);
        if border.top != edge {
            *border = BorderColor::all(edge);
            back.0 = Color::srgba(1.0, 1.0, 1.0, alpha * 0.25);
        }
    }
}
