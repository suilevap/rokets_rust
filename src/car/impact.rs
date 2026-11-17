//! Sparks when the car hits something (`Car.OnCollide`,
//! `EffectManager.CollideEffect`, `CreateCollisionSparks`).
//!
//! When a contact starts, the car's speed relative to what it hit, over
//! twice its top speed, is the blow's strength (0..1). Above 0.2 three
//! sparks fly off the contact point, away from what was hit, spread over
//! a half turn. Nothing when the car stood still, when the other body was
//! a sensor, or when the other was the faster one (its own hit).
//!
//! C# read the speeds as they were before the physics resolved the hit;
//! [`LastVelocity`] keeps them from the step before for the same reason.

use std::f32::consts::FRAC_PI_2;

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;

use super::Car;
use super::smoke::SmokeRng;
use crate::particles::styles::SPARKS;
use crate::render::particles::Particles;

/// The least blow that throws sparks (`pow > 0.2`).
const MIN_BLOW: f32 = 0.2;
/// Sparks per hit.
const SPARKS_PER_HIT: usize = 3;

/// A body's velocity before the latest physics step.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct LastVelocity(pub Vec2);

/// A hit by a car, `strength` 0..1, at a point, `away` pointing from what
/// was hit towards the car.
#[derive(Message, Debug, Clone, Copy)]
pub struct CarImpact {
    pub car: Entity,
    pub at: Vec2,
    pub away: Vec2,
    pub strength: f32,
}

/// Runs in `FixedUpdate`, before the physics step.
pub fn remember_velocity(mut bodies: Query<(&LinearVelocity, &mut LastVelocity)>) {
    for (v, mut last) in &mut bodies {
        last.0 = v.0;
    }
}

pub fn detect_impacts(
    mut started: MessageReader<CollisionStart>,
    collisions: Collisions,
    cars: Query<(&Car, &LastVelocity)>,
    others: Query<(Option<&LinearVelocity>, Has<Sensor>)>,
    mut impacts: MessageWriter<CarImpact>,
) {
    for event in started.read() {
        for (car_collider, car_body, other_collider, other_body) in [
            (event.collider1, event.body1, event.collider2, event.body2),
            (event.collider2, event.body2, event.collider1, event.body1),
        ] {
            let Some(car_entity) = car_body else {
                continue;
            };
            let Ok((car, last)) = cars.get(car_entity) else {
                continue;
            };
            let speed = last.0;
            let (other_v, sensor) = other_body
                .and_then(|b| others.get(b).ok())
                .map_or((Vec2::ZERO, false), |(v, s)| (v.map_or(Vec2::ZERO, |v| v.0), s));
            if speed == Vec2::ZERO || sensor || others.get(other_collider).is_ok_and(|(_, s)| s) {
                continue;
            }
            if speed.length_squared() < other_v.length_squared() {
                continue;
            }
            let strength = ((speed - other_v).length() / (car.params.max_speed * 2.0)).min(1.0);
            if strength <= MIN_BLOW {
                continue;
            }
            let Some(pair) = collisions.get(car_collider, other_collider) else {
                continue;
            };
            let Some(manifold) = pair.manifolds.first() else {
                continue;
            };
            let points = &manifold.points;
            if points.is_empty() {
                continue;
            }
            let at = points.iter().map(|p| p.point).sum::<Vec2>() / points.len() as f32;
            // The normal points from the pair's first collider to its second.
            let away = if pair.collider1 == car_collider { -manifold.normal } else { manifold.normal };
            impacts.write(CarImpact {
                car: car_entity,
                at,
                away,
                strength,
            });
        }
    }
}

pub fn impact_sparks(
    mut impacts: MessageReader<CarImpact>,
    mut rng: ResMut<SmokeRng>,
    mut particles: ResMut<Particles>,
) {
    let rng = &mut rng.0;
    for impact in impacts.read() {
        let dir = impact.away.to_angle();
        let mut at = impact.at;
        for _ in 0..SPARKS_PER_HIT {
            at += Vec2::new(rng.random::<f32>() * 2.0 - 1.0, rng.random::<f32>() * 2.0 - 1.0) * 0.5;
            let d = dir + (rng.random::<f32>() * 2.0 - 1.0) * FRAC_PI_2;
            let style = &SPARKS[rng.random_range(0..2)];
            particles.try_add(style, at, d, d);
        }
    }
}
