//! The Zombie's weapon (`ZombieWeapon`, 2013-2015): no gun, but a hold on
//! whoever is near, and a wave that stuns.
//!
//! C# origin: `ZombieWeapon.Update` (the look for a target, the bite and the
//! drag), `OnShoot` (the wave), `Draw` (the pull's red line), and the
//! numbers in [`super::model`] (`WeaponOptions.ZombieDefaultOptions`,
//! `ZombieDefaultOptionsPlayer`). `git log` of the C# file tells the
//! story: a drag and a bite that grew to hurt (2), a wave that stuns
//! (April 2015), a sound, and the longer reload for the enemies.
//!
//! - Every half second (`_checkEnemy`) the zombie, while alive, looks for
//!   the first car of another team, alive, within the weapon's range on both
//!   axes (4 units; "first" is the lowest map id here, the order
//!   `ControlableEntities` kept in C#). If it is the same one as at the last
//!   look and within the two radii plus one, a look counts towards a bite:
//!   at the presets' 0.5 s `ShootTime` every look bites, for 2
//!   ([`Hit`], from the zombie). A new one starts the count again; none
//!   leaves the count as it was; the target is whatever was found.
//! - Every step the target kept (until the next look, near or not) is
//!   dragged by a force of 36 towards the zombie, if it is still alive and
//!   a dynamic body.
//! - Its trigger (the player's fire, once the reload of 2 s is over, spending
//!   the whole energy store; an enemy's every 10 s, nothing spent) sends a
//!   [`ZombieWave`]: every enemy within twice the range is stunned for two
//!   seconds ([`crate::stun`]), a circle spreads from the zombie, `ZombieWave`
//!   sounds and the local player's view shakes.
//! - A flickering dark red line joins the zombie to its target
//!   (`Gradient2PixelOriginBegin`, a quarter to a whole unit thick, steady
//!   while the game is paused).
//!
//! Not ported:
//! - `GetZombieWeapon(level)` (damage by level for the spawn director), as
//!   with the Hammer's.
//! - The player zombie's `LocalDevice` is the [`LocalPlayer`] car for the
//!   shake (a held `NoneLocal` car or a hot-seat player differ: no hot seats
//!   here).
//! - `FakePosition` (the drawn position, interpolated, with the cabin's
//!   sway) is the car's drawn transform for the circle, the sound, the
//!   stun and the line; the tests of the target use the physics position
//!   as C#'s did.
//! - A car that is [`Inactive`] drags and bites nothing
//!   (`ControlableEntity.Update` is skipped for it); the other guns here
//!   do not check that, only this one does.

use avian2d::prelude::*;
use bevy::prelude::*;
use rand::Rng;

use super::model::{self, Kind, ZOMBIE_WAVE_REACH};
use super::{ViewRng, Weapon, WeaponSystems, fire_weapons};
use crate::actions::Inactive;
use crate::car::CarRadius;
use crate::car::control::Team;
use crate::controls::LocalPlayer;
use crate::explosion::{Health, Hit};
use crate::game::{GameState, LoadedMap, MapEntity, MapId};
use crate::particles::styles::{ZOMBIE_CIRCLE, ZOMBIE_CIRCLE_START};
use crate::render::camera::CameraShake;
use crate::render::layers::Layer;
use crate::render::particles::Particles;
use crate::render::sprite::{SpriteTint, UnitQuad, sprite_bundle};
use crate::sound::PlaySound;
use crate::stun::Stun;

/// The wave went off (`ZombieWeapon.OnShoot`): sent by the weapon's trigger.
#[derive(Message, Debug, Clone, Copy)]
pub struct ZombieWave {
    pub car: Entity,
}

/// The camera's jolt when the player's wave goes off (`Camera.Shake(0.2f)`).
const WAVE_SHAKE: f32 = 0.2;

/// `Gradient2PixelOriginBegin`: the pull's line.
const PULL_TEXTURE: &str = "Gradient2Pixel";
/// `Color.DarkRed`.
const DARK_RED: [f32; 4] = [139.0 / 255.0, 0.0, 0.0, 1.0];
/// The line's thickness while the game runs, a flicker between these
/// (`NextFloatFakeRange(0.25, 1.0)`), and paused (`0.75`).
const PULL_THICKNESS: (f32, f32) = (0.25, 1.0);
const PULL_THICKNESS_PAUSED: f32 = 0.75;

pub struct ZombiePlugin;

impl Plugin for ZombiePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ZombieWave>()
            .add_systems(
                FixedUpdate,
                (run_zombies, zombie_waves)
                    .chain()
                    .in_set(WeaponSystems)
                    .after(fire_weapons)
                    .after(super::hammer::run_saws)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(Update, draw_pulls.run_if(in_state(GameState::Playing)));
    }
}

/// `ZombieWeapon.Update`: the look, the bite and the drag.
#[allow(clippy::type_complexity)]
fn run_zombies(
    physics: Res<Time<Physics>>,
    mut zombies: Query<(Entity, &mut Weapon, &Position, &CarRadius, &Team, Option<&Health>), Without<Inactive>>,
    cars: Query<(Entity, &MapId, &Position, &CarRadius, &Team, &Health)>,
    mut bodies: Query<(&RigidBody, Forces)>,
    mut hits: MessageWriter<Hit>,
) {
    if physics.is_paused() {
        return;
    }
    for (owner, mut gun, position, radius, team, health) in &mut zombies {
        // `ControlableEntity.Update` is for the living.
        if gun.kind != Kind::Zombie || health.is_some_and(|h| h.hp <= 0) {
            continue;
        }
        let options = gun.options;
        if gun.zombie.needs_check() {
            // `IsCandidate(x, 1)`: alive, another team, near on both axes;
            // the first of the list.
            let found = cars
                .iter()
                .filter(|(_, _, at, _, other, hp)| {
                    hp.hp > 0 && other.0 != team.0 && model::zombie_reaches(position.0, at.0, &options, 1.0)
                })
                .min_by_key(|(_, id, ..)| id.0);
            let near = |at: Vec2, other: &CarRadius| model::bite_reach(position.0, at, radius.0, other.0);
            let bite = gun.zombie.look(&options, found.map(|c| c.0), |_| found.is_some_and(|c| near(c.2.0, c.3)));
            if bite && let Some((target, _, at, ..)) = found {
                hits.write(Hit {
                    target,
                    damage: options.damage,
                    from: (at.0 - position.0).to_angle(),
                    by: Some(owner),
                });
            }
        }
        // The drag: whoever was last found, while they live.
        if let Some(target) = gun.zombie.target
            && let Ok((_, _, at, _, _, _)) = cars.get(target)
            && let Ok((body, mut forces)) = bodies.get_mut(target)
            && body.is_dynamic()
        {
            forces.apply_force(model::zombie_pull(position.0, at.0, &options));
        }
    }
}

/// `ZombieWeapon.OnShoot`: stuns every enemy within twice the range; if the
/// zombie lives, the circle, the sound and the shake.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn zombie_waves(
    mut waves: MessageReader<ZombieWave>,
    owners: Query<(&Weapon, &Position, &Transform, &Team, Option<&Health>, Has<LocalPlayer>)>,
    cars: Query<(Entity, &Position, &Team, &Health)>,
    map: Res<LoadedMap>,
    mut stuns: MessageWriter<Stun>,
    mut particles: ResMut<Particles>,
    mut play: MessageWriter<PlaySound>,
    mut shake: ResMut<CameraShake>,
) {
    for wave in waves.read() {
        let Ok((gun, position, drawn, team, health, local)) = owners.get(wave.car) else {
            continue;
        };
        for (target, at, other, hp) in &cars {
            if hp.hp > 0 && other.0 != team.0 && model::zombie_reaches(position.0, at.0, &gun.options, ZOMBIE_WAVE_REACH) {
                stuns.write(Stun {
                    target,
                    seconds: model::STUN_TIME,
                });
            }
        }
        if health.is_some_and(|h| h.hp <= 0) {
            continue;
        }
        // `CreateZombieCircleEffect(FakePosition, FireDistance * 2)`.
        let at = drawn.translation.truncate();
        let radius = gun.options.range * ZOMBIE_WAVE_REACH;
        if let Some(circle) = map.sprites.get(ZOMBIE_CIRCLE.texture) {
            let rate = model::zombie_circle_rate(radius, circle.size.x);
            particles.add_scaled(&ZOMBIE_CIRCLE_START, at, 0.0, 0.0, rate);
            particles.add_scaled(&ZOMBIE_CIRCLE, at, 0.0, 0.0, rate);
        }
        play.write(PlaySound {
            name: "ZombieWave",
            at: Some(at),
            pitch: 0.0,
            volume: 1.0,
        });
        if local {
            shake.add(WAVE_SHAKE);
        }
    }
}

/// The line from a zombie to whom it drags (`ZombieWeapon.Draw`).
#[derive(Component)]
struct PullLine {
    owner: Entity,
}

/// `ZombieWeapon.Draw`: a dark red line from the zombie to its target, a
/// quarter to a whole unit thick and flickering.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draw_pulls(
    mut commands: Commands,
    physics: Res<Time<Physics>>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<ViewRng>,
    zombies: Query<(Entity, &Weapon, &Transform), Without<PullLine>>,
    targets: Query<(&Transform, &Health), Without<PullLine>>,
    mut lines: Query<(Entity, &PullLine, &mut Transform, &mut Visibility)>,
) {
    let Some(info) = map.sprites.get(PULL_TEXTURE) else {
        return;
    };
    // A line for each zombie; none for a car that has lost its weapon.
    let mut have = Vec::new();
    for (line, pull, mut transform, mut visibility) in &mut lines {
        let Ok((_, gun, car)) = zombies.get(pull.owner) else {
            commands.entity(line).despawn();
            continue;
        };
        have.push(pull.owner);
        let target = gun.zombie.target.and_then(|t| targets.get(t).ok()).filter(|(_, h)| h.hp > 0);
        let Some((to, _)) = target else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let (from, to) = (car.translation.truncate(), to.translation.truncate());
        let delta = to - from;
        let thickness = if physics.is_paused() {
            PULL_THICKNESS_PAUSED
        } else {
            rng.0.random_range(PULL_THICKNESS.0..PULL_THICKNESS.1)
        };
        transform.translation = ((from + to) / 2.0).extend(Layer::GroundParticles.z());
        transform.rotation = Quat::from_rotation_z(delta.to_angle());
        transform.scale = Vec3::new(delta.length(), thickness, 1.0);
        visibility.set_if_neq(Visibility::Inherited);
    }
    for (car, gun, _) in &zombies {
        if gun.kind != Kind::Zombie || have.contains(&car) {
            continue;
        }
        let tint = SpriteTint { color: DARK_RED, lit: true };
        commands.spawn((
            Name::new("Zombie pull"),
            MapEntity,
            PullLine { owner: car },
            Visibility::Hidden,
            sprite_bundle(info, 1.0, Layer::GroundParticles, tint, &quad, &mut materials),
        ));
    }
}
