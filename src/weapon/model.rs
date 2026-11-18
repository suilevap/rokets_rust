//! The weapon's numbers and timing, without the app: presets
//! (`WeaponOptions`, `DamageConst`), the smoothed aim and recoil
//! (`HardasticAngle`, `Hardastic`), the player's energy
//! (`EnergyComponent`) and the Minigun's firing rhythm (`MinigunWeapon`).
//!
//! Timers count seconds down while the game runs (C#'s `PeriodicUpdate`
//! compared absolute game times; the same while nothing pauses). One is
//! due once it is down to zero ([`due`]): C# counted 166 667 ticks a step,
//! so six steps were past 0.1 s and the Minigun fired ten bullets a second.

/// A timer counted down to `left` is due (a millionth of a second spare
/// for the float sums).
pub fn due(left: f32) -> bool {
    left <= 1e-6
}

use std::f32::consts::PI;

/// The weapons ported so far (`WeaponType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Minigun,
}

/// A weapon's numbers (`WeaponOptions`): world units, seconds, radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// `DamageMin`, the damage a bullet does (`Damage`).
    pub damage: i32,
    /// `BulletImpulse`.
    pub impulse: f32,
    /// Bullets fly within ± this of the aim (`Dispersion`).
    pub dispersion: f32,
    /// Bullets a trigger pull fires at most (`BulletsCount`).
    pub bullets: u32,
    /// Seconds between bullets (`ShootTime`).
    pub shoot_time: f32,
    /// Seconds before the trigger can be pulled again (`ReloadTime`).
    pub reload: f32,
    /// Energy a bullet costs the player (`EnergyConsumption`).
    pub energy: f32,
    /// How far the aim turns from straight ahead (`MaxAngle`); the player
    /// aims all round (`SetLocalControlPreferences`).
    pub max_angle: f32,
    /// Reach of the aim line (`FireDistance`).
    pub range: f32,
}

/// `DamageConst.Miningun`, `MiningunImpulse`.
const MINIGUN_DAMAGE: i32 = 3;
const MINIGUN_IMPULSE: f32 = 6.0;

/// `WeaponOptions.MinigunDefaultOptions`: the enemies' Minigun.
pub const MINIGUN: Options = Options {
    damage: MINIGUN_DAMAGE,
    impulse: MINIGUN_IMPULSE,
    dispersion: PI / 18.0,
    bullets: 10,
    shoot_time: 0.1,
    reload: 1.4,
    energy: 0.1,
    max_angle: PI / 3.0,
    range: 32.0,
};

/// `MinigunPlayerDefaultOptions`: the player's, two bullets a pull, a
/// pull every 0.1 s, half the spread, 1 damage, 0.04 energy a bullet
/// (0.03 in easy mode, not ported).
pub const MINIGUN_PLAYER: Options = Options {
    damage: 1,
    impulse: 6.0,
    dispersion: PI / 36.0,
    bullets: 2,
    shoot_time: 0.1,
    reload: 0.1,
    energy: 0.04,
    max_angle: PI / 3.0,
    range: 32.0,
};

/// A `WeaponType` name from `Cars.xml` and its numbers, for the player
/// (`local`) or not (`WeaponFactory`: the player gets the player preset
/// when the type has one).
pub fn for_type(name: &str, local: bool) -> Option<(Kind, Options)> {
    let pick = |ai: Options, player: Options| if local { player } else { ai };
    let options = match name {
        "Minigun" => pick(MINIGUN, MINIGUN_PLAYER),
        "MinigunLowConsumption" => pick(MINIGUN, Options { energy: 0.02, ..MINIGUN_PLAYER }),
        "Minigun360" => pick(Options { max_angle: PI, ..MINIGUN }, MINIGUN_PLAYER),
        "Minigun180" => pick(Options { max_angle: PI / 2.0, ..MINIGUN }, MINIGUN_PLAYER),
        // One preset for everyone, the energy left at the default 0.1.
        "MinigunSlowInacurate" => Options {
            dispersion: PI / 9.0,
            bullets: 5,
            shoot_time: 0.2,
            reload: 2.0,
            ..MINIGUN
        },
        _ => return None,
    };
    Some((Kind::Minigun, options))
}

/// `DamageConst.MinigunOptimalImpulseRate`, `MinigunMinImpulseRate`,
/// `MinigunProbabilityOptimalImuplse`: one bullet in five (and the last
/// one the energy allows) pushes 3×, the others a quarter.
pub const STRONG_IMPULSE: f32 = 3.0;
pub const WEAK_IMPULSE: f32 = 0.25;
pub const STRONG_CHANCE: f32 = 1.0 / 5.0;

/// `MathHelper.DirectionDiff`: `to − from`, the short way round.
pub fn direction_diff(to: f32, from: f32) -> f32 {
    let d = (to - from) % (2.0 * PI);
    if d.abs() > PI { -d.signum() * (2.0 * PI - d.abs()) } else { d }
}

/// `MathHelper.NormalizeAngle`: into −π..π.
pub fn normalize_angle(a: f32) -> f32 {
    direction_diff(a, 0.0)
}

/// `Hardastic`: moves `k` × the gap per second towards its destination,
/// there at once when a step is longer than 1/k.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ease {
    pub destination: f32,
    pub current: f32,
    pub k: f32,
}

impl Ease {
    pub fn new(value: f32, k: f32) -> Ease {
        Ease {
            destination: value,
            current: value,
            k,
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.current += (self.destination - self.current) * self.k * dt;
        if dt * self.k > 1.0 {
            self.current = self.destination;
        }
    }

    /// `HardasticAngle`: the same for an angle, the short way round.
    pub fn step_angle(&mut self, dt: f32) {
        self.current += direction_diff(self.destination, self.current) * self.k * dt;
        if dt * self.k > 1.0 {
            self.current = self.destination;
        }
    }
}

/// The aim relative to the car, with its recoil (`BaseComplexWeapon`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    /// Radians from straight ahead.
    pub angle: Ease,
    /// The gun kicked back along its barrel, C# pixels (`_feedback`).
    pub recoil: Ease,
    /// Seconds until an unused aim swings back ahead
    /// (`_returnAimToForwardDirection`, 4 s).
    pub return_in: f32,
    pub max_angle: f32,
    /// The player's: turns faster (k 9, not 4) and all the way round.
    pub local: bool,
}

/// `_returnAimToForwardDirection`.
pub const RETURN_AFTER: f32 = 4.0;
/// Pixels a bullet kicks the gun back (`_feedback.CurrentValue += 2`).
pub const RECOIL: f32 = 2.0;

impl Aim {
    pub fn new(max_angle: f32, local: bool) -> Aim {
        Aim {
            angle: Ease::new(0.0, if local { 9.0 } else { 4.0 }),
            recoil: Ease::new(0.0, 3.0),
            return_in: 0.0,
            max_angle,
            local,
        }
    }

    /// `Aim(target)`: turn towards world angle `to` of a car facing `car`.
    pub fn aim(&mut self, to: f32, car: f32) {
        let mut angle = normalize_angle(to - car);
        if !self.local {
            angle = angle.clamp(-self.max_angle, self.max_angle);
        }
        self.angle.destination = angle;
        self.return_in = RETURN_AFTER;
    }

    /// One of `BaseComplexWeapon.Update`'s eases. The car ran it twice a
    /// frame (from its `Update` and its `DrawCabina`); the caller does too.
    pub fn step(&mut self, dt: f32) {
        self.return_in -= dt;
        if due(self.return_in) {
            self.angle.destination = 0.0;
            self.return_in = RETURN_AFTER;
        }
        self.angle.step_angle(dt);
        self.recoil.step(dt);
    }
}

/// `EnergyComponent`: the player's shots draw on a store of 1 that refills
/// at 0.5 a second once a second has passed without spending; running
/// dry overloads it until it is back over half. Others' weapons spend
/// nothing (`!LocalDevice`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Energy {
    pub current: f32,
    pub overloaded: bool,
    /// What was last asked for, shown on the bar (`_lastCheck`).
    pub last_check: f32,
    /// Seconds until it refills again (`_rechargePeriod`).
    recharge_in: f32,
    /// Seconds the full bar still shows, fading (`_showFullTime`).
    pub full_shown: f32,
    /// The bar's trailing mark, easing to `max(current, last_check)`
    /// (`_previousValue`).
    pub trail: Ease,
    /// The bar's shake: its strength, easing from 1 (`_shakeHardastic`),
    /// and how far (`_shakeAmount`, pixels).
    pub shake: Ease,
    pub shake_amount: f32,
}

pub const MAX_ENERGY: f32 = 1.0;
const RECHARGE_PER_SEC: f32 = 0.5;
const RECHARGE_DELAY: f32 = 1.0;
/// Overloaded until over this (`_cooldown`).
const COOLDOWN: f32 = 0.5;
/// `_showFullTime`.
pub const SHOW_FULL: f32 = 3.0;

impl Default for Energy {
    fn default() -> Self {
        Energy {
            current: MAX_ENERGY,
            overloaded: false,
            last_check: 0.25,
            recharge_in: 0.0,
            full_shown: 0.0,
            trail: Ease::new(1.0, 6.0),
            shake: Ease::new(0.0, 6.0),
            shake_amount: 0.0,
        }
    }
}

impl Energy {
    fn shake(&mut self, amount: f32) {
        if amount > self.shake.current * self.shake_amount {
            self.shake.current = 1.0;
            self.shake_amount = amount;
        }
    }

    /// `CheckIfAvalaible`: is there `amount` to spend?
    pub fn check(&mut self, amount: f32) -> bool {
        if self.overloaded {
            return false;
        }
        let ok = self.current >= amount;
        self.last_check = amount;
        if !ok {
            self.shake(4.0);
        }
        ok
    }

    /// `TrySpend`: spend `amount`, or overload trying.
    pub fn try_spend(&mut self, amount: f32) -> bool {
        if amount == 0.0 {
            return true;
        }
        if self.overloaded {
            return false;
        }
        self.last_check = amount;
        if self.current >= amount {
            self.current -= amount;
            self.recharge_in = RECHARGE_DELAY;
            self.shake(10.0 * amount);
            true
        } else {
            self.shake(4.0);
            if !self.overloaded {
                self.last_check = self.last_check.max(COOLDOWN);
                self.overloaded = true;
            }
            false
        }
    }

    /// `Update`.
    pub fn step(&mut self, dt: f32) {
        self.recharge_in -= dt;
        self.full_shown -= dt;
        if self.current < MAX_ENERGY {
            self.full_shown = SHOW_FULL;
            if due(self.recharge_in) {
                self.current += RECHARGE_PER_SEC * dt;
                if self.current > MAX_ENERGY {
                    self.current = MAX_ENERGY;
                    self.shake(10.0);
                }
                if self.overloaded && self.current > COOLDOWN {
                    self.overloaded = false;
                }
            }
        }
        self.trail.destination = self.current.max(self.last_check);
        self.trail.step(dt);
    }
}

/// Spends energy, or not: the player's [`Energy`], or `None` for a weapon
/// that spends nothing.
fn spend(energy: &mut Option<&mut Energy>, amount: f32) -> bool {
    energy.as_mut().is_none_or(|e| e.try_spend(amount))
}

fn has(energy: &mut Option<&mut Energy>, amount: f32) -> bool {
    energy.as_mut().is_none_or(|e| e.check(amount))
}

/// The Minigun's rhythm (`MinigunWeapon`): pulling the trigger (at most
/// every `reload`) keeps it firing for `bullets × shoot_time`, a bullet
/// every `shoot_time` while the energy lasts.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Minigun {
    pub active: bool,
    /// Seconds the pull keeps it firing (`_shootDuration`).
    active_left: f32,
    /// Seconds to the next bullet (`ShootTime`).
    bullet_in: f32,
    /// Seconds before the trigger takes again (`_reload`).
    reload_in: f32,
    /// Seconds before another misfire click (`_missfirePeriod`).
    misfire_in: f32,
}

/// What a Minigun step did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shot {
    /// A bullet, pushing `impulse_rate` × the weapon's impulse, `spread`
    /// (−1..1) × its dispersion off the aim.
    Bullet { impulse_rate: f32, spread: f32 },
    /// Out of energy: a click (`Missfire`), at `pitch`.
    Misfire { pitch: f32 },
}

impl Minigun {
    /// The weapon's first pull waits a random part of its reload
    /// (`PeriodicUpdate.Sparse`); `offset` is 0..1.
    pub fn new(options: &Options, offset: f32) -> Minigun {
        Minigun {
            reload_in: options.reload * offset,
            ..Minigun::default()
        }
    }

    /// `TryShoot` with `ShootMultiple` held: starts (or keeps) it firing.
    pub fn pull(&mut self, options: &Options, mut energy: Option<&mut Energy>) {
        // The pull itself costs nothing (`_onShootEnergyConsumption = 0`).
        if due(self.reload_in) && spend(&mut energy, 0.0) {
            self.reload_in = options.reload;
            self.active = true;
            self.active_left = options.bullets as f32 * options.shoot_time;
        }
    }

    /// `Update`: `random` gives numbers 0..1 (for the impulse, the spread
    /// and the misfire), in that order.
    pub fn step(
        &mut self,
        dt: f32,
        options: &Options,
        mut energy: Option<&mut Energy>,
        mut random: impl FnMut() -> f32,
    ) -> Option<Shot> {
        self.reload_in -= dt;
        self.bullet_in -= dt;
        self.misfire_in -= dt;
        self.active_left -= dt;
        if !self.active {
            return None;
        }
        let mut shot = None;
        let mut ok = true;
        if due(self.bullet_in) {
            ok = spend(&mut energy, options.energy);
            if ok {
                // The last bullet the energy allows pushes hard too.
                let last = !has(&mut energy, options.energy);
                let strong = last || random() < STRONG_CHANCE;
                shot = Some(Shot::Bullet {
                    impulse_rate: if strong { STRONG_IMPULSE } else { WEAK_IMPULSE },
                    spread: random() * 2.0 - 1.0,
                });
                self.bullet_in = options.shoot_time;
            } else if due(self.misfire_in) {
                // `Update(Time + random × 0.5 s)`: 0.3 s and up to 0.5 s more.
                self.misfire_in = random() * 0.5 + 0.3;
                shot = Some(Shot::Misfire { pitch: random() * 0.2 });
            }
        }
        if !ok || due(self.active_left) {
            self.active = false;
        }
        shot
    }

    /// `CanShoot`: the trigger would take now (asking the energy, as C#
    /// did, sets its last-asked mark to nothing).
    pub fn can_pull(&self, mut energy: Option<&mut Energy>) -> bool {
        due(self.reload_in) && has(&mut energy, 0.0)
    }

    /// `Cancel`.
    pub fn cancel(&mut self) {
        self.active = false;
    }
}
