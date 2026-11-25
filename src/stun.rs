//! A car held helpless for a while (`StunComponent`, April 2015, for the
//! Zombie's wave).
//!
//! C# origin: `StunComponent` (`Init`, `Update`, `Draw`) and
//! `ParticleSystemManager.CreateStunEffect`.
//!
//! - A [`Stun`] message (the Zombie's wave sends one per enemy it reaches)
//!   makes a living car [`Stunned`] for its seconds, with a star flying in
//!   and spinning up over it ([`STUN_FLASH`]).
//! - While it lasts, every step the car's stick, aim, trigger and look are
//!   zero (`CurrentAxisControl`, `WeaponAxisControl`, `Fire`): it drives
//!   on nothing, and fires nothing. This runs after whatever wrote the
//!   intent ([`IntentSystems`]) and before the cars drive; the player's
//!   devices write theirs before the step. The gadget is not touched, as
//!   in C#. The stun is over at the first step after its time, which
//!   still held it (`NeedUpdate` was asked after the control was cleared).
//! - A star, a rotating, pulsing sprite (`Particles/Stun`, 3 radii wide
//!   at most), is drawn over the car meanwhile.
//!
//! Not ported:
//! - Stuns stack in C# (each `AddComponent` was its own component with
//!   its own timer and star; the car was free when the last was over, a
//!   star drawn for each). Here a stun already on the car starts over with
//!   the new time (the car is free when the newest is over, as it was) and
//!   shows one star.
//! - The star's flying-in particle ([`STUN_FLASH`]) is the car's drawn
//!   position (`FakePosition`), as the other effects here.
//! - `StunComponent.Destroy` is empty, and a car that dies is simply freed
//!   (its view goes with its parts).

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::car::CarRadius;
use crate::controls::{IntentSystems, PlayerIntent};
use crate::explosion::Health;
use crate::game::{GameState, LoadedMap};
use crate::particles::styles::STUN_FLASH;
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::weapon::WeaponSystems;
use crate::weapon::model::due;

/// Stuns a car for `seconds` (`new StunComponent(TimeSpan)` added to it).
#[derive(Message, Debug, Clone, Copy)]
pub struct Stun {
    pub target: Entity,
    pub seconds: f32,
}

/// A car that cannot be driven or fire.
#[derive(Component, Debug, Clone, Copy)]
pub struct Stunned {
    /// Seconds left (`_disableTimer`).
    pub left: f32,
}

impl Stunned {
    /// `NeedUpdate`: time passes; true when it is over.
    pub fn step(&mut self, dt: f32) -> bool {
        self.left -= dt;
        due(self.left)
    }
}

/// What the stun does to the control (`CurrentAxisControl`,
/// `WeaponAxisControl` and `Fire` cleared; the gadget stays).
pub fn hold(intent: &mut PlayerIntent) {
    intent.move_dir = Vec2::ZERO;
    intent.aim_dir = Vec2::ZERO;
    intent.look = Vec2::ZERO;
    intent.fire = false;
    intent.fire_accurate = false;
}

/// The star drawn over a stunned car.
#[derive(Component)]
struct StunView;

/// `Particles/Stun`: the star's sprite.
const STUN_TEXTURE: &str = "Particles/Stun";

pub struct StunPlugin;

impl Plugin for StunPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Stun>()
            .add_systems(
                FixedUpdate,
                (
                    apply_stuns
                        .after(IntentSystems)
                        .before(crate::car::drive_cars),
                    begin_stuns.after(WeaponSystems),
                )
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, (show_stuns, end_stun_views).run_if(in_state(GameState::Playing)));
    }
}

/// `StunComponent.Init`: the car is stunned and the star flies in.
fn begin_stuns(
    mut commands: Commands,
    mut stuns: MessageReader<Stun>,
    cars: Query<(&Health, &Transform), With<PlayerIntent>>,
    mut particles: ResMut<Particles>,
) {
    for stun in stuns.read() {
        let Ok((health, at)) = cars.get(stun.target) else {
            continue;
        };
        if health.hp <= 0 {
            continue;
        }
        commands.entity(stun.target).insert(Stunned { left: stun.seconds });
        particles.add_scaled(&STUN_FLASH, at.translation.truncate(), 0.0, 0.0, 1.0);
    }
}

/// `StunComponent.Update`: the control is cleared; when the time is out,
/// the stun goes.
fn apply_stuns(
    mut commands: Commands,
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    mut cars: Query<(Entity, &mut Stunned, Option<&mut PlayerIntent>, Option<&Health>)>,
) {
    let dt = time.delta_secs();
    if physics.is_paused() || dt <= 0.0 {
        return;
    }
    for (car, mut stunned, intent, health) in &mut cars {
        // A car that has died is free.
        if health.is_none_or(|h| h.hp <= 0) {
            commands.entity(car).remove::<Stunned>();
            continue;
        }
        if let Some(mut intent) = intent {
            hold(&mut intent);
        }
        if stunned.step(dt) {
            commands.entity(car).remove::<Stunned>();
        }
    }
}

/// `PhaseHelper.Triangle_Period500`: 0 to 1 and back once a second.
fn triangle(seconds: f32) -> f32 {
    let r = (seconds % 1.0) * 2.0;
    if r > 1.0 { 2.0 - r } else { r }
}

/// `StunComponent.Draw`: the star over the car, turning once a second
/// (`Sawtooth_Period1000`, clockwise: negated here), pulsing between 0.8
/// and 1 of a size 1.5 times the car's width (its radius times two).
#[allow(clippy::type_complexity)]
fn show_stuns(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stunned: Query<(Entity, &CarRadius, &Transform, Option<&Children>), (With<Stunned>, Without<StunView>)>,
    mut views: Query<&mut Transform, With<StunView>>,
) {
    let Some(info) = map.sprites.get(STUN_TEXTURE) else {
        return;
    };
    let t = time.elapsed_secs();
    for (car, radius, car_at, children) in &stunned {
        let size = 2.0 * radius.0 * 1.5 * (0.8 + 0.2 * triangle(t));
        // In the world's frame, the car's own turn taken off.
        let angle = -(t % 1.0) * std::f32::consts::TAU - car_at.rotation.to_euler(EulerRot::ZYX).0;
        let existing = children.into_iter().flatten().copied().find(|c| views.contains(*c));
        if let Some(mut view) = existing.and_then(|c| views.get_mut(c).ok()) {
            view.scale = Vec3::new(size, size, 1.0);
            view.rotation = Quat::from_rotation_z(angle);
            continue;
        }
        let lit = SpriteTint { color: [1.0; 4], lit: true };
        let mut sprite = sprite_bundle(info, 1.0, Layer::AirParticles, lit, &quad, &mut materials);
        sprite.2.scale = Vec3::new(size, size, 1.0);
        sprite.2.rotation = Quat::from_rotation_z(angle);
        commands.entity(car).with_children(|parent| {
            parent.spawn((Name::new("Stun"), StunView, sprite));
        });
    }
}

/// The star goes when the stun does.
fn end_stun_views(
    mut commands: Commands,
    mut ended: RemovedComponents<Stunned>,
    cars: Query<&Children>,
    views: Query<(), With<StunView>>,
) {
    for car in ended.read() {
        for child in cars.get(car).into_iter().flatten() {
            if views.contains(*child) {
                commands.entity(*child).despawn();
            }
        }
    }
}
