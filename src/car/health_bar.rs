//! The health bar over a hurt car (`ControlableEntity.DrawHealth`,
//! `_drawHealth`).
//!
//! A car nobody's devices drive shows a bar above it for two seconds after
//! it takes a blow it survives; each new blow starts the two seconds
//! again, and the bar fades out over them. The player's car has none
//! (`!Control.LocalDevice`), nor does one that is dead.
//!
//! The bar is a black strip 3 radii wide and 0.15 radii tall, 1.1 radii
//! above the car's middle (up the screen, whichever way the car faces),
//! with the fill over it: 2.875 radii wide at most and 0.1 tall, the share
//! of the health that is left, from red (whole) to yellow (nearly gone)
//! (`Color.Lerp(Yellow, Red, share)`, as C# had it). While the car has any
//! shield, the fill is the shield's share, in white. Drawn unlit and over
//! the world (`DefaultWorldForeground`).
//!
//! A car whose shield is `Immortal` (`SetImmortal`) shows none, whether or
//! not the shield has points left: `!Control.LocalDevice && (Shield == null
//! || !Shield.Immortal)`.

use bevy::prelude::*;

use super::shield::Shield;
use crate::explosion::{Damaged, Health};
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::render::layers::Layer;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};

/// `_drawHealthTimer`: seconds the bar shows after a blow.
pub const SHOWN_FOR: f32 = 2.0;
/// The bar's size in radii: the back, the fill (`Radius * 96f` and
/// `Radius * 92f` pixels wide, `4.8` and `3.2` tall, of 32 to the unit).
const BACK: Vec2 = Vec2::new(96.0 / 32.0, 4.8 / 32.0);
const FILL: Vec2 = Vec2::new(92.0 / 32.0, 3.2 / 32.0);
/// How far the fill lies over the back (it draws after it).
const FILL_LIFT: f32 = 0.0005;
/// How far above the car's middle, in radii.
const ABOVE: f32 = 1.1;

/// A car that shows its health bar when hurt.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct HealthBar {
    /// The car's radius (`Entity.Radius`), world units.
    pub radius: f32,
    /// Seconds left to show; none: hidden.
    pub left: f32,
}

/// What the bar looks like: where, how wide and which colour (sRGB, alpha
/// the fade), for a car of `radius` at `at` with `share` of its health, or
/// of its shield, left, `left` seconds before the bar goes. The back's
/// size and the fill's, and the fill's colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub centre: Vec2,
    pub back: Vec2,
    pub fill: Vec2,
    pub fill_colour: [f32; 3],
    pub alpha: f32,
}

impl Look {
    /// `DrawHealth`'s numbers. `shield`: the shield's share when it has
    /// any left (shown instead, white). `up` is the screen's up in the
    /// world (turned on portrait screens), where the bar goes.
    pub fn of(at: Vec2, radius: f32, health: f32, shield: Option<f32>, left: f32, up: Vec2) -> Look {
        let (share, fill_colour) = match shield {
            Some(part) => (part, [1.0, 1.0, 1.0]),
            // `Color.Lerp(Color.Yellow, Color.Red, share)`.
            None => (health, [1.0, 1.0 - health, 0.0]),
        };
        Look {
            centre: at + up * radius * ABOVE,
            back: BACK * radius,
            fill: Vec2::new(FILL.x * share, FILL.y) * radius,
            fill_colour,
            alpha: (left / SHOWN_FOR).clamp(0.0, 1.0),
        }
    }
}

/// One of a car's two bar quads.
#[derive(Component, Debug)]
struct BarPart {
    car: Entity,
    fill: bool,
}

pub struct HealthBarPlugin;

impl Plugin for HealthBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (show_on_damage, spawn_bars, place_bars)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

/// A blow the car survived starts the two seconds again (`TakeDamage`).
fn show_on_damage(mut damaged: MessageReader<Damaged>, mut bars: Query<&mut HealthBar>) {
    for hit in damaged.read() {
        if !hit.immortal
            && let Ok(mut bar) = bars.get_mut(hit.target)
        {
            bar.left = if hit.killed { 0.0 } else { SHOWN_FOR };
        }
    }
}

/// Two quads for each new bar, not tied to the car: they must not turn
/// with it.
fn spawn_bars(
    mut commands: Commands,
    new: Query<Entity, Added<HealthBar>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(pixel) = map.sprites.get("WhitePixel") else {
        return;
    };
    for car in &new {
        for fill in [false, true] {
            let tint = SpriteTint {
                color: [0.0, 0.0, 0.0, 0.0],
                lit: false,
            };
            commands.spawn((
                Name::new("Health bar"),
                MapEntity,
                BarPart { car, fill },
                sprite_bundle(pixel, 1.0, Layer::HealthBar, tint, &quad, &mut materials),
                Visibility::Hidden,
            ));
        }
    }
}

#[allow(clippy::type_complexity)]
fn place_bars(
    mut commands: Commands,
    time: Res<Time>,
    mut cars: Query<(&mut HealthBar, &Health, Option<&Shield>, &Transform), Without<BarPart>>,
    mut parts: Query<(Entity, &BarPart, &mut Transform, &mut SpriteTint, &mut Visibility)>,
    view: Res<crate::render::camera::ViewRotation>,
) {
    // Over the car and level on the screen, whichever way it is turned
    // (C# had no portrait view: world up was always the screen's).
    let up = view.screen_to_world(Vec2::Y);
    let turn = Quat::from_rotation_z(up.to_angle() - std::f32::consts::FRAC_PI_2);
    for (e, part, mut transform, mut tint, mut visibility) in &mut parts {
        // The car is gone, or dead (it has no more health bar then).
        let Ok((mut bar, health, shield, car)) = cars.get_mut(part.car) else {
            commands.entity(e).despawn();
            continue;
        };
        // Both parts of a bar tick it down: once, by the back.
        if !part.fill {
            bar.left = (bar.left - time.delta_secs()).max(0.0);
        }
        if bar.left <= 0.0 || health.hp <= 0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let shield_part = shield.filter(|s| s.points() > 0).map(Shield::part);
        let share = health.hp as f32 / health.max.max(1) as f32;
        let look = Look::of(car.translation.truncate(), bar.radius, share, shield_part, bar.left, up);
        let size = if part.fill { look.fill } else { look.back };
        transform.translation.x = look.centre.x;
        transform.translation.y = look.centre.y;
        // The fill a hair over the back, so it is drawn after it.
        transform.translation.z = Layer::HealthBar.z() + if part.fill { FILL_LIFT } else { 0.0 };
        transform.rotation = turn;
        transform.scale = size.extend(1.0);
        let [r, g, b] = if part.fill { look.fill_colour } else { [0.0; 3] };
        tint.color = [r, g, b, look.alpha];
        *visibility = Visibility::Inherited;
    }
}
