//! The Laser (`LaserWeapon`, 2012-2016): a beam that hurts what it touches
//! for as long as the trigger is held; the `Laser`, `LaserNoReflection` and
//! `LaserWithDelay` types.
//!
//! C# origin: `LaserWeapon` (`TryShoot` through `BaseComplexWeapon`,
//! `OnShoot`, `Update`, `NoShoot`, `Cancel`, `LaserVisiblityUpdate`,
//! `UpdateLaser`, `DrawFlash`), `LaserRenderer.Render` (the beam's path and
//! its drawing; the path is [`beam::trace`], shared with the Railgun's
//! bullet and the aim line), `LaserView`, `DelayWeaponDecorator`,
//! `WeaponFactory`'s three registrations, `WeaponOptions.LaserDefaultOptions`
//! and `DamageConst.Laser` (the numbers are in [`super::model`]). The
//! story, by the C# history: the beam began as the aim line itself (the
//! `LaserRenderer` of `BaseComplexWeapon`) that also hurt what it touched;
//! then it bounced (`ReflectionCount`, 2014), `LaserNoReflection` and the
//! turrets of `Garage` and `TheGate` came, and in 2026 `LaserWithDelay`: the
//! turret shows a red line that brightens for 0.9 s before the beam comes
//! (`DelayWeaponDecorator`; the `TestLaserDelay` map).
//!
//! - A beam is the ray from the gun's mount to the muzzle's line, up to 256
//!   units in all (`_fireDistance`, the `Total` reach: each bounce counts
//!   from where it is drawn), bounced twice (`Laser`; none for the others)
//!   off what cannot be hurt and not head-on ([`beam::trace`] with
//!   `stop_at_hurt`: the beam ends at a car or a box that can be hurt).
//! - While the trigger is held (`FireType.ShootMultiple`: either fire
//!   flag with the delay, only the held one without; [`Laser::trigger`])
//!   the gun is active: the beam is blue-violet, 2 to 8 pixels wide (new
//!   every frame, so it flickers; up to 3 it is the thin noise strip with
//!   its glow, wider a solid bar with a wide soft blur), a flash sits at the
//!   muzzle and one at every end, the gun kicks back 1 to 5 pixels (new
//!   every step) and the `Laser` sound plays.
//! - Every step an active beam hurts everything it touches for 3, all its
//!   legs, the car's own team too (`DamageEntity`, no `ignoreDamage`: the
//!   test map sets its turrets to one team "so they don't damage each
//!   other" for that reason), 180 a second; it does not push. Hurt uses what
//!   the last frame drew (`CollideFixtures` of the last `Render`), so the
//!   step the beam appears is free: [`Laser::update`]. A spark flies off
//!   an end one time in twenty a step ([`LaserSpark`], the view's matter).
//! - A leg that ends on a car held for the player's devices
//!   ([`LocalDevice`]) flashes the dirty glass red, one frame in four
//!   (`LaserRenderer.Render`: `CreateLaserEffect`, only on a map with the
//!   lens): a quarter of the way along the leg, shaken by half a unit
//!   ([`laser_crossings`], a [`GlassFlash`]; the flash is `render/dirty_lens`'s).
//!   The aim line of a gun that has not fired yet (the charging red line)
//!   flashes the same, as C# drew it with the same renderer.
//! - Not firing, the beam is gone; the gun shows no line at all (`NoShoot`
//!   clears `EnableWeaponLineDraw`), except while a delayed gun charges: a
//!   thin red line, as bright as the charge is far (0.1 to 1), and a red
//!   ring closing on the muzzle ([`LaserCharge`], `_chargingStyle`).
//! - The delay ([`Laser`] with a `charge` time; `DelayWeaponDecorator`): a
//!   pull while it is not charged starts a charge and shows the line; the
//!   charge goes on, whatever the trigger does, until a pull finds it
//!   complete (0.9 s on); then the gun is "warm" and fires straight
//!   through, and stays so while it is let go for less than 0.9 s; let
//!   go longer, it starts over. The shoot pattern pulls the trigger a
//!   little early for such a gun (`PeriodicFireControl`'s pre-fire,
//!   [`crate::ai::ShootPattern`]).
//!
//! Not ported:
//! - `LaserCollectorButton.LaserCollide`: the beam would press such a
//!   button (`CreateButton` of `ButtonType` `LaserCollector`, `TheGate`);
//!   the button is not ported (see `button`), so the beam touches it as it
//!   does any wall.
//! - `LaserRenderer.Render`'s `CreateLaserEffect` for the other guns: in
//!   C# every weapon's aim line makes the flash where it ends on the
//!   player's car (the Railgun's bounces too, even the player's own after
//!   it has bounced); only this gun's beam does here, the others draw no
//!   line (see `render/dirty_lens` for the flash itself).
//! - `Camera.IsInView`: the beam, flashes and sparks are made wherever
//!   they are (as the Tesla's).
//! - The gun's `Entity.Visible` that hides its flash; the beam, a dead
//!   car's included, goes with the `Weapon` (the wreck takes it away, as
//!   with the other guns), where C# `Cancel`led it.
//! - The second weapon `Update` of `DrawCabina` (drawing a second time
//!   with the cabin's offset) does not damage; here [`super::Weapon::place`]
//!   rides the cabin once a step, as for the other guns.
//! - The beam's width and the flashes' scales and brightness come from
//!   `RandomHelper`'s fake generator (visual); here from the view's own
//!   seeded generator, a frame at a time. The kick is a gameplay draw.
//! - The player has no Laser (`Cars.xml` gives none, no map's `SetWeapon`
//!   names it for the player); its energy (0.001 a step) is spent all the
//!   same.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use avian2d::prelude::*;
use bevy::math::Affine2;
use bevy::prelude::*;
use rand::Rng;

use super::beam::{self, BeamRules, Leg, Reach};
use super::model::{Kind, due};
use super::view::{beam_look, paint};
use super::{Hurt, PassedByBullets, ViewRng, Weapon, WeaponSystems, collision_sparks, fire_weapons};
use crate::actions::Inactive;
use crate::car::control::LocalDevice;
use crate::explosion::Hit;
use crate::game::{GameState, LoadedMap, MapEntity};
use crate::particles::styles::LASER_CHARGE;
use crate::render::dirty_lens::{CROSSING_CHANCE, CROSSING_SHAKE, Flash, GlassFlash};
use crate::render::layers::Layer;
use crate::render::particles::{FlashBoost, Particles};
use crate::render::sprite::UnitQuad;
use crate::units::PX_PER_UNIT;

/// The most legs a beam draws (a start and bounces; the presets bounce twice).
pub const MAX_LEGS: usize = 4;
/// `RandomEvent(0.05f)`: a step's chance of a spark at an end.
pub const SPARK_CHANCE: f32 = 0.05;

/// What the beam draws with, to load with a map that has one: the gun's
/// sprite, the strips and bar (shared with the aim line), the blur's
/// gradient, the muzzle's streak and the ends' flash.
pub const TEXTURES: [&str; 9] = [
    "Weapons\\Laser",
    "Special\\LaserLine",
    "Special\\LaserLine2",
    "WhitePixel",
    "Gradient8WidthPixelNonSym",
    BLUR,
    MUZZLE,
    END,
    "Particles\\Circle",
];

/// `Drawer.Gradient3PixelOriginTop`: a 3 × 1 gradient, bright in the middle.
const BLUR: &str = "Particles\\3Pixel";
/// `_flashView`: the streak out of the muzzle (127 × 16 pixels).
const MUZZLE: &str = "Particles\\minigunTrail";
/// `Drawer.FlashSprite` (32 × 32 pixels).
const END: &str = "Particles\\flash2";
const MUZZLE_SIZE: Vec2 = Vec2::new(127.0, 16.0);
const END_SIZE: f32 = 32.0;
/// `_flashView.Origin.X`: the streak turns about a point 95% along it.
const MUZZLE_PIVOT: f32 = 0.95;

/// `Color.BlueViolet`.
const BLUE_VIOLET: [f32; 3] = [138.0 / 255.0, 43.0 / 255.0, 226.0 / 255.0];

/// `OnShoot`'s kick: the gun is set back 1 to 5 pixels
/// (`NextFloatFakeRange(1, 5)`), `random` 0..1.
pub fn recoil(random: f32) -> f32 {
    1.0 + 4.0 * random
}

/// What a pull of the trigger did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Trigger {
    /// The gun is active now: it fires this step.
    pub fired: bool,
    /// A charge began (the ring closes on the muzzle).
    pub charge_started: bool,
}

/// `DelayWeaponDecorator`'s state.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Delay {
    /// `_isCharging`.
    charging: bool,
    /// `_isCharged`.
    charged: bool,
    /// The decorator's clock, seconds (`TimeManager.Time`).
    clock: f32,
    /// `_shootStart`.
    start: f32,
    /// `_noShootSince` (`None`: 0).
    quiet_since: Option<f32>,
}

/// The beam's state (`LaserWeapon`, with its decorator): the trigger and the
/// delay, what the gun shows and what the last frame drew.
#[derive(Clone, Debug)]
pub struct Laser {
    /// `_isActive`: fired and not let go.
    pub active: bool,
    /// `EnableWeaponLineDraw`: the line is wanted.
    pub line: bool,
    /// `_laserRender.Visible`: the beam (or the line) is drawn.
    pub visible: bool,
    /// `WeaponLineDrawOpacity`: how bright the charging line is.
    pub opacity: f32,
    /// A frame has drawn the beam since it came on (`CollideFixtures`
    /// are filled).
    shown: bool,
    /// `ChargeTime`, seconds; 0 without the decorator.
    charge: f32,
    delay: Delay,
    /// The legs the last frame drew, from the muzzle: what a step hurts.
    pub legs: Vec<Leg>,
}

impl Laser {
    /// A gun whose trigger waits `charge` seconds first (0: it does not).
    pub fn new(charge: f32) -> Laser {
        Laser {
            active: false,
            line: false,
            visible: false,
            opacity: 1.0,
            shown: false,
            charge,
            delay: Delay {
                charging: false,
                charged: false,
                clock: 0.0,
                start: 0.0,
                quiet_since: None,
            },
            legs: Vec::new(),
        }
    }

    /// `ChargeTime`.
    pub fn charge_time(&self) -> f32 {
        self.charge
    }

    /// `DelayWeaponDecorator.CanShoot` for a gun with no energy store: it
    /// is not charging.
    pub fn can_shoot(&self) -> bool {
        !(self.charge > 0.0 && self.delay.charging)
    }

    /// The start of a step (`ControlableEntity.ControlHandle`): time passes
    /// and the line is wanted while the gun is aimed (`EnableWeaponLineDraw
    /// = needAim`).
    pub fn begin(&mut self, dt: f32, aiming: bool) {
        self.delay.clock += dt;
        self.line = aiming;
    }

    /// `HandleControlEvent`: a trigger with either fire flag pulls it
    /// (`shoot`); the beam comes on only for the held one (`multiple`:
    /// `ShootMultiple`) and when `pay` (the energy) allows; no flag at all
    /// lets go. `charge_time > 0` puts the decorator in front.
    pub fn trigger(&mut self, shoot: bool, multiple: bool, pay: impl FnOnce() -> bool) -> Trigger {
        let mut out = Trigger::default();
        if shoot {
            let mut pass = true;
            if self.charge > 0.0 {
                let d = &mut self.delay;
                d.quiet_since = None;
                if !d.charged {
                    if !d.charging {
                        d.charging = true;
                        d.start = d.clock;
                        self.line = true;
                        out.charge_started = true;
                    }
                    let spent = d.clock - d.start;
                    self.opacity = (spent / self.charge).clamp(0.1, 1.0);
                    if due(self.charge - spent) {
                        d.charging = false;
                        d.charged = true;
                        self.opacity = 1.0;
                    } else {
                        pass = false;
                    }
                }
            }
            if pass && multiple && pay() {
                self.active = true;
                out.fired = true;
            }
        } else if self.let_go() {
            // `LaserWeapon.NoShoot`.
            self.active = false;
            self.visible = false;
            self.line = false;
        }
        out
    }

    /// The decorator's `NoShoot`: whether it goes on to the gun's own (a
    /// gun that is charging keeps its line and its charge).
    fn let_go(&mut self) -> bool {
        if self.charge <= 0.0 {
            return true;
        }
        let d = &mut self.delay;
        if d.charging {
            self.line = true;
            d.quiet_since = None;
            return false;
        }
        if d.charged {
            let since = *d.quiet_since.get_or_insert(d.clock);
            if due(self.charge - (d.clock - since)) {
                // A long silence: it starts over.
                d.charged = false;
                d.start = 0.0;
                d.quiet_since = None;
                self.opacity = 1.0;
                self.line = false;
            }
            // A short gap stays warm.
            return true;
        }
        d.quiet_since = None;
        d.start = 0.0;
        self.opacity = 1.0;
        self.line = false;
        true
    }

    /// `LaserWeapon.Update`: the line is shown while wanted
    /// (`LaserVisiblityUpdate`), the beam of an active gun always. Whether
    /// the beam hurts this step: it does when the last frame drew it
    /// (`_laserRendered`, the collisions it found).
    pub fn update(&mut self) -> bool {
        let wanted = self.line;
        // The collisions are cleared when the beam goes off and on again.
        let mut drawn = self.shown && wanted;
        self.visible = wanted;
        if self.active && !self.visible {
            self.visible = true;
            self.line = true;
            drawn = false;
        }
        self.active && drawn
    }

    /// The end of a step: the frame has drawn what is visible.
    pub fn end_step(&mut self) {
        self.shown = self.visible;
        if !self.visible {
            self.legs.clear();
        }
    }

    /// `Cancel`: the car was deactivated or killed.
    pub fn cancel(&mut self) {
        self.active = false;
        self.line = false;
        self.visible = false;
        self.shown = false;
        self.legs.clear();
    }
}

/// A flash of fire where the beam ends, for the view: its place and the
/// angle of the surface's normal (`CreateCollisionSparks(p, dir, 1)`).
#[derive(Message, Debug, Clone, Copy)]
pub struct LaserSpark {
    pub at: Vec2,
    pub normal: f32,
}

/// A delayed gun began to charge (`_chargingStyle`): where its muzzle is.
#[derive(Message, Debug, Clone, Copy)]
pub struct LaserCharge {
    pub at: Vec2,
}

pub struct LaserPlugin;

impl Plugin for LaserPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LaserSpark>()
            .add_message::<LaserCharge>()
            .add_systems(
                FixedUpdate,
                run_lasers
                    .in_set(WeaponSystems)
                    .after(fire_weapons)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (forget_laser_views, laser_effects, laser_crossings, draw_lasers)
                    .chain()
                    .after(WeaponSystems)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// A car that has lost its gun (`SetWeapon` with `None`) has lost its
/// beam's parts ([`draw_lasers`] despawns them); the mark must go too.
fn forget_laser_views(mut commands: Commands, cars: Query<Entity, (With<LaserViews>, Without<Weapon>)>) {
    for car in &cars {
        commands.entity(car).remove::<LaserViews>();
    }
}

/// The legs of a laser gun's beam, as `LaserRenderer.Render` found them.
pub fn beam_legs(gun: &Weapon, car: Entity, spatial: &SpatialQuery, passes: &dyn Fn(Entity) -> bool, hurt: &dyn Fn(Entity) -> bool) -> Vec<Leg> {
    let rules = BeamRules {
        reach: Reach::Total(gun.options.range),
        bounces: gun.options.reflections.min(MAX_LEGS as u32 - 1),
        stop_at_hurt: true,
    };
    beam::trace(spatial, car, passes, hurt, gun.muzzle(), gun.at, gun.direction, rules)
}

/// `LaserWeapon.Update` after the trigger: the beam hurts what the last
/// frame found it touching, then it is found again from where the gun is
/// now (the next frame's `Render`).
#[allow(clippy::too_many_arguments)]
pub fn run_lasers(
    time: Res<Time>,
    physics: Res<Time<Physics>>,
    spatial: SpatialQuery,
    mut guns: Query<(Entity, &mut Weapon, Has<Inactive>)>,
    skipped: Query<(), PassedByBullets>,
    hurt: Query<(), Hurt>,
    mut hits: MessageWriter<Hit>,
    mut sparks: MessageWriter<LaserSpark>,
) {
    if physics.is_paused() || time.delta_secs() <= 0.0 {
        return;
    }
    let (passes, can_hurt) = (|e| skipped.contains(e), |e| hurt.contains(e));
    for (car, mut gun, inactive) in &mut guns {
        if gun.kind != Kind::Laser || inactive {
            continue;
        }
        let gun = &mut *gun;
        if gun.laser.update() {
            for leg in &gun.laser.legs {
                let Some((target, normal)) = leg.hit else {
                    continue;
                };
                // `normal.GetAngle()`: nought for no normal.
                let normal = normal.to_angle();
                // `RUST_LOG=tilerider::weapon=debug`, in C#'s axes as `/trace`.
                debug!("laser hit {target:?} at {:.3} {:.3}", leg.end.x, -leg.end.y);
                sparks.write(LaserSpark { at: leg.end, normal });
                if hurt.contains(target) {
                    // `dir + MathHelper.PI`: into the surface.
                    hits.write(Hit {
                        target,
                        damage: gun.options.damage,
                        from: normal + PI,
                        by: Some(car),
                    });
                }
            }
        }
        gun.laser.legs = if gun.laser.visible {
            beam_legs(gun, car, &spatial, &passes, &can_hurt)
        } else {
            Vec::new()
        };
        gun.laser.end_step();
    }
}

/// The sparks off the ends and the charge's ring, with the views' random
/// numbers.
fn laser_effects(
    mut sparks: MessageReader<LaserSpark>,
    mut charges: MessageReader<LaserCharge>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<ViewRng>,
) {
    for spark in sparks.read() {
        if rng.0.random::<f32>() < SPARK_CHANCE {
            collision_sparks(&mut particles, &mut rng.0, spark.at, spark.normal);
        }
    }
    for charge in charges.read() {
        particles.try_add(&LASER_CHARGE, charge.at, 0.0, 0.0);
    }
}

/// Where along a leg a crossing flash is made, from where its ray starts
/// (`startPos + (endPos - startPos) * 1f / 4f`).
pub const CROSSING_ALONG: f32 = 0.25;

/// Where the glass flashes for a leg of a beam that ends on the player's
/// car, for `shake` of 0..1 on each axis (`CreateLaserEffect`:
/// `NextVector2Fake() * 0.5f` from `p`).
pub fn crossing_at(leg: &Leg, shake: Vec2) -> Vec2 {
    leg.cast_from + (leg.end - leg.cast_from) * CROSSING_ALONG + (shake * 2.0 - Vec2::ONE) * CROSSING_SHAKE
}

/// `LaserRenderer.Render`'s `CreateLaserEffect`: a frame a visible beam's
/// leg ends on a car held for the player's devices, a chance in four of a
/// red flash of the glass, with the view's random numbers.
fn laser_crossings(
    guns: Query<&Weapon>,
    locals: Query<(), With<LocalDevice>>,
    mut rng: ResMut<ViewRng>,
    mut flashes: MessageWriter<GlassFlash>,
) {
    for gun in &guns {
        if gun.kind != Kind::Laser || !gun.laser.visible {
            continue;
        }
        for leg in &gun.laser.legs {
            let Some((target, _)) = leg.hit else {
                continue;
            };
            if !locals.contains(target) || rng.0.random::<f32>() >= CROSSING_CHANCE {
                continue;
            }
            let shake = Vec2::new(rng.0.random::<f32>(), rng.0.random::<f32>());
            flashes.write(GlassFlash {
                at: crossing_at(leg, shake),
                flash: Flash::Laser,
            });
        }
    }
}

/// A gun whose beam has its views.
#[derive(Component)]
struct LaserViews;

/// One quad of a beam.
#[derive(Component)]
struct LaserPart {
    owner: Entity,
    kind: Part,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    /// The line of leg `i`.
    Core(usize),
    /// Its faint wider copy beside the thin line.
    Glow(usize),
    /// The soft blur of a wide beam.
    Blur(usize),
    /// The flash out of the muzzle.
    Muzzle,
    /// The flash at the end of leg `i`.
    End(usize),
}

/// What a frame draws of one gun.
struct Look {
    owner: Entity,
    active: bool,
    /// The beam's width in pixels.
    width: f32,
    color: Color,
    legs: Vec<Leg>,
    muzzle: Vec2,
    direction: f32,
    /// The muzzle flash's scale and brightness.
    streak: (Vec2, f32),
    /// The end flashes' scales.
    ends: [f32; MAX_LEGS],
}

fn srgb(c: [f32; 3], alpha: f32) -> Color {
    Color::srgba(c[0], c[1], c[2], alpha)
}

/// `LaserRenderer.Render`, `LaserWeapon.UpdateLaser` and `DrawFlash`, a
/// frame at a time.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_lasers(
    mut commands: Commands,
    time: Res<Time>,
    boost: Res<FlashBoost>,
    map: Res<LoadedMap>,
    quad: Res<UnitQuad>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    (mut images, mut premultiplied): (ResMut<Assets<Image>>, ResMut<crate::render::premultiply::Premultiplied>),
    mut rng: ResMut<ViewRng>,
    guns: Query<(Entity, &Weapon, Has<LaserViews>)>,
    mut parts: Query<(Entity, &LaserPart, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
) {
    let t = time.elapsed_secs();
    let rng = &mut rng.0;
    let mut looks: Vec<Look> = Vec::new();
    for (car, gun, has_views) in &guns {
        if gun.kind != Kind::Laser {
            // Its parts go with the beam (below); the mark with them, so a
            // later Laser (`SetWeapon`) gets its parts again.
            if has_views {
                commands.entity(car).remove::<LaserViews>();
            }
            continue;
        }
        if !has_views {
            commands.entity(car).insert(LaserViews);
            for leg in 0..MAX_LEGS {
                for kind in [Part::Core(leg), Part::Glow(leg), Part::Blur(leg), Part::End(leg)] {
                    spawn_part(&mut commands, &mut materials, &quad, car, kind);
                }
            }
            spawn_part(&mut commands, &mut materials, &quad, car, Part::Muzzle);
        }
        let laser = &gun.laser;
        if !laser.visible || laser.legs.is_empty() {
            continue;
        }
        // `UpdateLaser`: a flickering blue-violet beam while it fires, a
        // red line of the charge's brightness before.
        let (width, color) = if laser.active {
            (2.0 + 6.0 * rng.random::<f32>(), srgb(BLUE_VIOLET, 1.0))
        } else {
            (3.0, Color::srgba(1.0, 0.0, 0.0, laser.opacity))
        };
        // `DrawFlash`.
        let streak = (
            Vec2::new(0.5 + 0.25 * rng.random::<f32>(), 1.25 + 0.75 * rng.random::<f32>()),
            0.5 + 0.5 * rng.random::<f32>(),
        );
        let mut ends = [0.0; MAX_LEGS];
        for end in &mut ends {
            *end = 0.75 + 1.25 * rng.random::<f32>();
        }
        looks.push(Look {
            owner: car,
            active: laser.active,
            width,
            color,
            legs: laser.legs.clone(),
            muzzle: gun.muzzle(),
            direction: gun.direction,
            streak,
            ends,
        });
    }
    for (entity, part, mut transform, mut visibility, material) in &mut parts {
        if !guns.get(part.owner).is_ok_and(|(_, g, _)| g.kind == Kind::Laser) {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(look) = looks.iter().find(|l| l.owner == part.owner) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let Some(placed) = place_part(part.kind, look, t) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let Some(mut m) = materials.get_mut(&material.0) else {
            continue;
        };
        let image = map.sprites.get(placed.texture).map(|s| premultiplied.of(&s.image, &mut images));
        paint(&mut m, image, placed.color, boost.0, placed.uv);
        *transform = placed.transform;
        visibility.set_if_neq(Visibility::Inherited);
    }
}

fn spawn_part(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, quad: &UnitQuad, owner: Entity, kind: Part) {
    let material = materials.add(StandardMaterial {
        alpha_mode: AlphaMode::Add,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        depth_bias: Layer::Glow.depth_bias(),
        ..default()
    });
    commands.spawn((
        Name::new("Laser"),
        MapEntity,
        LaserPart { owner, kind },
        Mesh3d(quad.0.clone()),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Hidden,
    ));
}

/// Where and how one quad of a beam is drawn this frame.
struct Placed {
    texture: &'static str,
    color: Color,
    uv: Affine2,
    transform: Transform,
}

/// A quad of `size` world units turned to `angle`, its middle at `at`.
fn quad_at(at: Vec2, angle: f32, size: Vec2) -> Transform {
    Transform::from_translation(at.extend(Layer::Glow.z()))
        .with_rotation(Quat::from_rotation_z(angle))
        .with_scale(size.extend(1.0))
}

/// `LaserRenderer.Render`'s drawing of one leg, `DrawFlash`'s of the
/// flashes; none for a quad the beam has no use for now.
fn place_part(kind: Part, look: &Look, t: f32) -> Option<Placed> {
    let tint = |scale: f32| {
        let c = look.color.to_srgba();
        Color::srgba(c.red * scale, c.green * scale, c.blue * scale, c.alpha)
    };
    match kind {
        Part::Core(i) | Part::Glow(i) | Part::Blur(i) => {
            let leg = look.legs.get(i)?;
            let (start, end) = (leg.start, leg.end);
            let length = start.distance(end);
            if length <= 0.0 {
                return None;
            }
            let along = (end - start).to_angle();
            let mid = (start + end) / 2.0;
            let thin = look.width <= 3.0;
            match kind {
                Part::Core(_) => {
                    let (texture, w, uv) = beam_look(look.width, length, leg.hit.is_some(), false, t);
                    Some(Placed {
                        texture,
                        color: look.color,
                        uv,
                        transform: quad_at(mid, along, Vec2::new(length, w / PX_PER_UNIT)),
                    })
                }
                Part::Glow(_) if thin => {
                    let (texture, w, uv) = beam_look(look.width, length, leg.hit.is_some(), true, t);
                    Some(Placed {
                        texture,
                        color: tint(0.5),
                        uv,
                        transform: quad_at(mid, along, Vec2::new(length, w / PX_PER_UNIT)),
                    })
                }
                // `IsDrawBlur`: `Gradient3PixelOriginTop` (3 pixels across)
                // stretched to 7.5 × the width, along the whole leg.
                Part::Blur(_) if !thin => {
                    let c = look.color.to_srgba();
                    Some(Placed {
                        texture: BLUR,
                        color: Color::srgba(c.red * 0.75, c.green * 0.75, c.blue * 0.75, c.alpha * 0.75),
                        uv: Affine2::IDENTITY,
                        transform: quad_at(mid, along - FRAC_PI_2, Vec2::new(3.0 * 7.5 * look.width / PX_PER_UNIT, length)),
                    })
                }
                _ => None,
            }
        }
        Part::Muzzle => {
            if !look.active {
                return None;
            }
            let (scale, bright) = look.streak;
            let size = MUZZLE_SIZE * scale;
            // Turned about a point 95% along it, to the muzzle's far side.
            let forward = Vec2::from_angle(look.direction);
            let middle = look.muzzle + forward * ((MUZZLE_PIVOT - 0.5) * size.x / PX_PER_UNIT);
            Some(Placed {
                texture: MUZZLE,
                color: Color::srgba(bright, bright, bright, bright),
                uv: Affine2::IDENTITY,
                transform: quad_at(middle, look.direction + PI, size / PX_PER_UNIT),
            })
        }
        Part::End(i) => {
            let leg = look.legs.get(i)?;
            if !look.active || leg.hit.is_none() {
                return None;
            }
            let side = END_SIZE * look.ends[i] / PX_PER_UNIT;
            Some(Placed {
                texture: END,
                color: Color::srgba(0.75, 0.75, 0.75, 0.75),
                uv: Affine2::IDENTITY,
                transform: quad_at(leg.end, FRAC_PI_4, Vec2::splat(side)),
            })
        }
    }
}
