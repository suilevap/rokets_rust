//! The player's aim help: the aim leans towards the enemy it points near.
//!
//! C# origin: `BaseComplexWeapon.UpdateAimMagnet` and the lerp in `Aim`
//! (2016; `Settings.EnableAimHelp`, on). For the player's gun only (a
//! `LocalDevice` control that is `IsVisibleForAutoAim`), every half second
//! (and at once when its candidate is dead) it looks for a *candidate
//! magnet*:
//!
//! - an enemy car: alive, of another team, `IsVisibleForAutoAim` (not the
//!   `None*` controls, nor one `AIAdjustment` hid), on the screen
//!   (`IsInView`: its middle within its radius of the picture), within the
//!   gun's range on both axes, and in sight (`MapManager.LineCollide`);
//! - whose direction from the car is within a cosine of 0.75 of the aim
//!   (the aim as the player gave it, whatever its length, dotted with the
//!   unit direction: a short stick or a near cursor weighs less), scored by
//!   `distance × (1 − cosine / 2)` and the least wins, under the gun's
//!   range ([`pick`]).
//!
//! Every time the gun is aimed, if it has a candidate, the aim becomes
//! `lerp(aim, direction to the candidate from the gun, 0.75)` ([`pulled`]):
//! three quarters of the way to it. Dead candidates are dropped at the next
//! look; one gone from the map is simply not followed.
//!
//! Not ported: the debug square over the candidate; the C# `Update` that
//! made the look twice a frame for a car with a cabin (the half second
//! period made that no difference); the aim's last direction
//! (`_destinationSource`) is the last the player gave, as here, one step
//! late.

use bevy::math::Vec2;
use bevy::prelude::*;

use avian2d::prelude::{Physics, PhysicsTime, Position};

use super::model::{closer_by_chebyshev, due};
use super::Weapon;
use crate::car::CarRadius;
use crate::car::control::{AutoAimVisible, Team};
use crate::controls::{LocalPlayer, NoInput};
use crate::explosion::Health;
use crate::game::LoadedMap;
use crate::render::camera::MainCamera;

/// `Settings.EnableAimHelp`.
pub const ENABLED: bool = true;
/// `_candidateMagnetValue`: how far the aim is pulled.
pub const PULL: f32 = 0.75;
/// `_candidateMagnetCosMin`.
pub const COS_MIN: f32 = 0.75;
/// `_candidateMagnetPeriod`, s.
pub const PERIOD: f32 = 0.5;

/// The gun's candidate magnet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AimMagnet {
    pub candidate: Option<Entity>,
    /// Seconds to the next look.
    left: f32,
    /// The aim as the player last gave it (`_destinationSource`).
    pub source: Vec2,
}

impl AimMagnet {
    /// Time passes; whether to look for a candidate now: the period is up
    /// (and starts again), or the candidate is dead.
    pub fn due(&mut self, dt: f32, candidate_alive: bool) -> bool {
        self.left -= dt;
        let period = due(self.left);
        if period {
            self.left = PERIOD;
        }
        period || (self.candidate.is_some() && !candidate_alive)
    }
}

/// `UpdateAimMagnet`'s `GetBest`: among `candidates` (entity and middle,
/// already alive, visible, of another team, in view, in range and in
/// sight), the one the aim `aim` points nearest to from `from`, if any is
/// within `range`.
pub fn pick(candidates: impl IntoIterator<Item = (Entity, Vec2)>, from: Vec2, aim: Vec2, range: f32) -> Option<Entity> {
    let mut best: Option<(f32, Entity)> = None;
    for (entity, at) in candidates {
        let delta = at - from;
        let length = delta.length();
        let amount = (delta / length).dot(aim);
        // A candidate on top of the car has no direction (`NaN > 0.75`).
        let score = if amount > COS_MIN { length * (1.0 - amount * 0.5) } else { f32::MAX };
        if score < best.map_or(range, |b| b.0) {
            best = Some((score, entity));
        }
    }
    best.map(|b| b.1)
}

/// `Aim`'s lerp: the aim `target` pulled towards the candidate at
/// `candidate`, seen from the gun at `gun`.
pub fn pulled(target: Vec2, gun: Vec2, candidate: Vec2) -> Vec2 {
    let delta = candidate - gun;
    if delta == Vec2::ZERO {
        return target;
    }
    target.lerp(delta.normalize(), PULL)
}

/// `IsInView`, for a camera: the middle of something of `radius` within
/// the picture (a margin of its radius all round).
pub fn in_view(camera: &Camera, transform: &GlobalTransform, at: Vec2, radius: f32) -> bool {
    let project = |p: Vec2| camera.world_to_ndc(transform, p.extend(0.0));
    let (Some(middle), Some(edge_x), Some(edge_y)) = (
        project(at),
        project(at + Vec2::X * radius),
        project(at + Vec2::Y * radius),
    ) else {
        return false;
    };
    let margin_x = (edge_x.x - middle.x).abs().max((edge_y.x - middle.x).abs());
    let margin_y = (edge_x.y - middle.y).abs().max((edge_y.y - middle.y).abs());
    middle.x.abs() <= 1.0 + margin_x && middle.y.abs() <= 1.0 + margin_y
}

/// `BaseComplexWeapon.Update`'s call of `UpdateAimMagnet`, for the player's
/// gun.
#[allow(clippy::type_complexity)]
pub(super) fn update_aim_magnets(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    map: Res<LoadedMap>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut guns: Query<(&mut Weapon, &Position, &Team, &Health, &AutoAimVisible), (With<LocalPlayer>, Without<NoInput>)>,
    cars: Query<(Entity, &Team, &Position, &AutoAimVisible, &CarRadius, &Health)>,
) {
    let dt = time.delta_secs();
    if !ENABLED || physics.is_paused() || dt <= 0.0 {
        return;
    }
    let camera = cameras.single().ok();
    for (mut gun, position, team, health, visible) in &mut guns {
        // `LaserWeapon` clears `_enableAimHelp`.
        if health.hp <= 0 || !visible.0 || gun.kind == super::model::Kind::Laser {
            continue;
        }
        let alive = gun.magnet.candidate.is_none_or(|c| cars.get(c).is_ok_and(|car| car.5.hp > 0));
        if !gun.magnet.due(dt, alive) {
            continue;
        }
        let (from, range, aim) = (position.0, gun.options.range, gun.magnet.source);
        let candidates = cars.iter().filter_map(|(entity, other, at, visible, radius, health)| {
            let wanted = health.hp > 0
                && visible.0
                && other.0 != team.0
                && camera.is_none_or(|(c, t)| in_view(c, t, at.0, radius.0))
                && closer_by_chebyshev(from, at.0, range)
                && !map.line_collides(from, at.0);
            wanted.then_some((entity, at.0))
        });
        gun.magnet.candidate = pick(candidates, from, aim, range);
    }
}
