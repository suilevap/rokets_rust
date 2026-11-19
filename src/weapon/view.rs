//! What the gun looks like: its sprite on the cabin, the aim line
//! (`LaserRenderer`, `LaserView`) and the energy bar
//! (`EnergyComponent.DrawOnUI`).
//!
//! The aim line runs from the muzzle to the first thing a bullet would
//! hit, up to the gun's range. Normally it is a thin green line (3 C#
//! pixels) of a noise strip (`Special\LaserLine`) whose rows scroll once
//! a second, so it shimmers, with a wider faint glow (`LaserLine2`, the
//! `ImprovedLaserView` the desktop had). While the gun cannot fire (out
//! of energy, or between pulls), it is white, pulsing once a second and
//! up to 8 pixels wide: solid when it ends on something, fading out at
//! its end otherwise. Drawing it asked the energy whether it could fire
//! at all (`CheckEnergy`), which reset the bar's last-asked mark; so
//! does this.
//!
//! The energy bar sits under the top edge, 144 × 4 pixels at 800 × 480
//! (scaled with the screen as `UISizeMultiplier`): a faint track, the
//! energy in white, the energy just spent trailing behind in red
//! (flashing red and yellow while it is short of what was asked), and a
//! jolt when it is spent, runs dry or fills. Full, it fades out over
//! 3 s.
//!
//! Not ported: the reload look (`SetLaserReload`, for weapons that fire
//! only accurately, as the Railgun); the laser's sparkles on the
//! player's own car; the extra offset C# gave the bar in hot-seat games
//! (`PlayerUIOffset`).

use avian2d::prelude::*;
use bevy::math::Affine2;
use bevy::prelude::*;
use rand::Rng;

use super::model::{Energy, MAX_ENERGY, SHOW_FULL};
use super::{
    BULLET_REACH, MINIGUN_ORIGIN, PassedByBullets, MINIGUN_SIZE, MINIGUN_SPRITE, PlayerEnergy, Weapon, WeaponSystems,
};
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

/// The aim line's two quads: the line and its glow.
#[derive(Component)]
struct AimLine {
    glow: bool,
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
    guns: Query<(Entity, Option<&Children>), With<Weapon>>,
    views: Query<(), With<GunView>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (car, children) in &guns {
        if children.is_some_and(|c| c.iter().any(|e| views.contains(e))) {
            continue;
        }
        let Some(info) = map.sprites.get(MINIGUN_SPRITE) else {
            continue;
        };
        let lit = SpriteTint {
            color: [1.0; 4],
            lit: true,
        };
        let mut sprite = sprite_bundle(info, 1.0, Layer::Weapon, lit, &quad, &mut materials);
        // From the turning point to the sprite's middle (texture y down).
        let centre = MINIGUN_SIZE / 2.0 - MINIGUN_ORIGIN;
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
    mut lines: Query<(&AimLine, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
) {
    if lines.is_empty() {
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
                AimLine { glow },
                Mesh3d(quad.0.clone()),
                MeshMaterial3d(material),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
        return;
    }
    let shown = guns.iter_mut().next().and_then(|(car, gun, intent, energy)| {
        // `LaserVisiblityUpdate`: while aiming, not while the gadget charges.
        if !gun.aiming || intent.gadget {
            return None;
        }
        // `MinigunWeapon.UpdateLaser`: low while it cannot fire and is not firing.
        let can_fire = gun.minigun.can_pull(energy.map(|e| &mut e.into_inner().0));
        let low = !can_fire && !gun.minigun.active;
        let start = gun.muzzle();
        let dir = Vec2::from_angle(gun.direction);
        let filter = SpatialQueryFilter::from_excluded_entities([car]);
        let hit = Dir2::new(dir).ok().and_then(|d| {
            spatial.cast_ray_predicate(gun.at, d, gun.options.range.min(BULLET_REACH), true, &filter, &|e| {
                !skipped.contains(e)
            })
        });
        let end = hit.map_or(gun.at + dir * gun.options.range, |h| gun.at + dir * h.distance);
        Some((start, end, low, hit.is_some()))
    });
    let t = time.elapsed_secs();
    for (line, mut transform, mut visibility, material) in &mut lines {
        let Some((start, end, low, hit)) = shown else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let length = start.distance(end);
        if length <= 0.0 || (low && line.glow) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Some(mut m) = materials.get_mut(&material.0) else {
            continue;
        };
        let (texture, width, color, uv) = if low {
            let tri = triangle(t);
            let texture = if hit { SOLID } else { FADING };
            // The fading strip runs from its solid end at the muzzle.
            let uv = if hit {
                Affine2::IDENTITY
            } else {
                Affine2::from_scale_angle_translation(Vec2::new(-1.0, 1.0), 0.0, Vec2::new(1.0, 0.0))
            };
            (texture, 3.0 + 5.0 * (1.0 - tri), Color::srgba(1.0, 1.0, 1.0, 0.1 + 0.9 * tri), uv)
        } else {
            // A row of the strip, as long a part of it as the line is of 32.
            let rows = if line.glow { 33.0 } else { 16.0 };
            let row = ((t % 1.0) * 15.0).floor();
            let row = if line.glow { row * 2.0 + 1.0 } else { row };
            let part = ((length / STRIP_LENGTH).min(1.0) * 128.0).floor().max(1.0) / 128.0;
            let uv = Affine2::from_scale_angle_translation(Vec2::new(part, 1.0 / rows), 0.0, Vec2::new(0.0, row / rows));
            // `Color.Green` is (0, 128, 0); the glow at half.
            let (w, k) = if line.glow { (12.0, 0.5) } else { (3.0, 1.0) };
            (if line.glow { LASER_GLOW } else { LASER_LINE }, w, Color::srgba(0.0, 128.0 / 255.0 * k, 0.0, 1.0), uv)
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
