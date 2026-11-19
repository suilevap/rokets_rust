//! What the behaviour trees read and write: `AiBlackboard`.
//!
//! C# origin: `AiBlackboard` (2013–2016), the `BT.IBlackboard` every
//! `AIBTBaseControl` tree runs on: the car's `Target`, `LastDamager` (and
//! when), the `State` flags (`SeeTarget`, `WillSeeTarget`), `ScanDistance`,
//! `FireAccuracy`, the one timer (`StartTimer`, `Timer`) and the world it
//! asks (`Entity.Position`, `Entity.Weapon`, `GetEnemies`,
//! `MapManager.LineCollide`), and `AIBTBaseControl`'s `_aiUpdateTime`.
//!
//! Here the blackboard is a component per AI car and the context of its
//! tree (flatbt's blackboard: the tree reads and writes only it). The world
//! cannot be asked from inside a tree, so [`AiBlackboard::gather`] copies in,
//! before the tree runs, what C# asked: the car's own position, heading, team
//! and gun; the target (always) and, when the tree is to think, the damager
//! and the enemies within `ScanDistance` with whether each is in sight now
//! and in 0.5 s (the two `LineCollide`s a think needs, nothing is cast on the
//! steps that only tick). What the tree remembers (target, damager, flags,
//! timer, random stream) lives in it.
//!
//! `gather` also decides how the tree is entered this step ([`Tick`]), which
//! is where `BTLib`'s running rules meet flatbt-bevy's tick; see [`super`]. And
//! it runs the blackboard's own `Update` ([`AiBlackboard::update`]) before a
//! thinking step: the target is looked at again and a damager that is gone or
//! hurt it 5 s ago is forgotten.
//!
//! Not ported: `AiBlackboard`'s other states (`SeeTargetForward`,
//! `LeaderSeeTarget` (always off for a turret: no leader), `TakeDamage`), its
//! `Leader`, `Orders`, paths (`Path`, `PatrolPath`), `Traits`,
//! `LastKnowTargetPosition`, `AdditionalTargetPosition`, `ActionData`,
//! `MapActionOnTirgger`, the custom data, `Blocker`, `PathPeriodControl`:
//! the turret's tree reads none of them (they come with the trees that do).
//! `Draw`'s debug overlay. A target that is deactivated (`IsActive` false)
//! counts as gone at once; C# keeps it (alive) as the target until it is out
//! of sight. `GetPredictionPosition`'s correction of a prediction that lands
//! inside a wall (it picks one of the first 16 of the map's free points, an
//! arbitrary place far off): `WillSeeTarget` looks at the plain prediction.

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use flatbt_bevy::Tick;

use crate::weapon::model::{FireRange, closer_by_chebyshev, due};

/// `AIBTBaseControl._aiUpdateTime`: how often the tree is thought over, s.
pub const THINK_PERIOD: f32 = 0.25;
/// `AIBTBaseControl._scanDistance`: how far a car looks for enemies.
pub const SCAN_DISTANCE: f32 = 16.0;
/// `IsTragetVisible(0.5f)`: `WillSeeTarget` looks this far ahead, s.
pub const WILL_SEE_AHEAD: f32 = 0.5;
/// `AiBlackboard._remeberDamagerTime`, s.
pub const DAMAGER_MEMORY: f32 = 5.0;

/// A car the trees may look at: alive and active, with what they need to
/// tell (`ControlableEntity`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foe {
    pub entity: Entity,
    pub team: u8,
    pub pos: Vec2,
    pub vel: Vec2,
    /// `Entity.Radius`.
    pub radius: f32,
}

/// The car's gun, as the trees need it (`Entity.Weapon`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gun {
    /// A gun that aims (`IBaseComplexWeapon`), not the Hammer.
    pub complex: bool,
    /// `MaxAngle`, after `AdjustWeaponMaxAngle`.
    pub max_angle: f32,
    /// What `TargetInFireRange` asks.
    pub range: FireRange,
}

/// A car in sight of the blackboard's, as of the last time the tree thought.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub foe: Foe,
    /// `!LineCollide(position, foe.pos)`.
    pub visible: bool,
    /// `!LineCollide(position, foe.pos + foe.vel * 0.5)`.
    pub visible_soon: bool,
}

/// The car's own, as the blackboard reads it off the car each step
/// (`Entity.Position`, …).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pos: Vec2,
    /// World radians.
    pub heading: f32,
    pub team: u8,
    /// `None` for a car with no gun.
    pub gun: Option<Gun>,
}

/// `AiBlackboard`, one per AI car.
#[derive(Component, Debug, Clone)]
pub struct AiBlackboard {
    /// `ScanDistance`: how far it looks for enemies.
    pub scan_distance: f32,
    /// `FireAccuracy`: 1 fires at what is in the line, more at what is near
    /// it.
    pub fire_accuracy: f32,
    /// The car's position (`Entity.Position`), world units, y up.
    pub pos: Vec2,
    /// The car's heading (`Entity.Direction`), world radians.
    pub heading: f32,
    /// `Entity.Control.Team`.
    pub team: u8,
    /// `Entity.Weapon`; `None` for a car with no gun.
    pub gun: Option<Gun>,
    /// The cars the tree can ask about: the target, and on a thinking step
    /// the damager and the enemies within `ScanDistance`.
    seen: Vec<Seen>,
    /// `Target`: who it fights, or looked at last.
    target: Option<Entity>,
    /// `States.SeeTarget`, as of the last [`update`](Self::update).
    sees: bool,
    /// `States.WillSeeTarget`.
    will_see: bool,
    /// `LastDamager`.
    damager: Option<Entity>,
    /// Seconds until the damager is forgotten (`LastDamagerTime` against
    /// `_remeberDamagerTime`).
    damager_left: f32,
    /// `Time.TotalGameTime`, s.
    now: f64,
    /// `_startTimer`.
    timer_start: f64,
    /// `_aiUpdateTime`: seconds to the next forced think.
    think_in: f32,
    /// How the tree is entered this step; what flatbt-bevy's tick asks.
    pub tick: Tick,
    /// `RandomHelper`: this car's own stream.
    rng: SmallRng,
}

impl AiBlackboard {
    /// A blackboard with its own random stream, starting from `seed`. The
    /// first think comes at a random moment within the period
    /// (`PeriodicUpdate.Sparse`), so cars are out of step.
    pub fn new(seed: u64) -> AiBlackboard {
        let mut rng = SmallRng::seed_from_u64(seed);
        let think_in = rng.random::<f32>() * THINK_PERIOD;
        AiBlackboard {
            scan_distance: SCAN_DISTANCE,
            fire_accuracy: 1.0,
            pos: Vec2::ZERO,
            heading: 0.0,
            team: 0,
            gun: None,
            seen: Vec::new(),
            target: None,
            sees: false,
            will_see: false,
            damager: None,
            damager_left: 0.0,
            now: 0.0,
            timer_start: 0.0,
            think_in,
            // Not entered until gathered.
            tick: Tick::Skip,
            rng,
        }
    }

    /// What the tree reads of the world, copied in before it runs for a step
    /// of `dt`; and how it is entered.
    ///
    /// `running` is whether the car's tree has an invocation saved
    /// (`Context.LastRunningNode != null`: flatbt-bevy's act is on the car).
    /// `cars` are all the cars alive and active (this one too: it is no enemy
    /// of itself); `blocked` says whether a wall is in the way
    /// (`MapManager.LineCollide`).
    ///
    /// The tree is thought over (`Context.Think`: `Evaluate`, after
    /// [`update`](Self::update)) every 0.25 s and whenever nothing runs; the
    /// other steps only tick the running action (`Context.Run`: `Resume`).
    pub fn gather(&mut self, dt: f32, running: bool, body: Body, cars: &[Foe], blocked: &dyn Fn(Vec2, Vec2) -> bool) {
        self.now += dt as f64;
        self.damager_left -= dt;
        self.pos = body.pos;
        self.heading = body.heading;
        self.team = body.team;
        self.gun = body.gun;
        self.think_in -= dt;
        let forced = due(self.think_in);
        if forced {
            self.think_in = THINK_PERIOD;
        }
        let think = forced || !running;
        self.tick = if think { Tick::Evaluate } else { Tick::Resume };
        let (pos, scan, team, target, damager) = (self.pos, self.scan_distance, self.team, self.target, self.damager);
        let wanted = |f: &&Foe| {
            Some(f.entity) == target
                || (think && (Some(f.entity) == damager || (f.team != team && closer_by_chebyshev(f.pos, pos, scan))))
        };
        let mut seen = std::mem::take(&mut self.seen);
        seen.clear();
        seen.extend(cars.iter().filter(wanted).map(|&foe| {
            // Only a think asks about sight; the flags stay as they were between.
            let (visible, visible_soon) = if think {
                (!blocked(pos, foe.pos), !blocked(pos, foe.pos + foe.vel * WILL_SEE_AHEAD))
            } else {
                (false, false)
            };
            Seen { foe, visible, visible_soon }
        }));
        self.seen = seen;
        if think {
            self.update();
        }
    }

    /// `RandomHelper.NextFloat`: 0 up to 1.
    pub fn random(&mut self) -> f32 {
        self.rng.random()
    }

    /// `StartTimer`.
    pub fn start_timer(&mut self) {
        self.timer_start = self.now;
    }

    /// `Timer.TotalSeconds`: since the last [`start_timer`](Self::start_timer).
    pub fn timer(&self) -> f32 {
        (self.now - self.timer_start) as f32
    }

    /// Whom it is attacking, or looked at last.
    pub fn target(&self) -> Option<Entity> {
        self.target
    }

    /// `Entity_OnTakeDamage`: whoever hurt it is remembered for 5 s.
    pub fn hurt_by(&mut self, who: Entity) {
        self.damager = Some(who);
        self.damager_left = DAMAGER_MEMORY;
    }

    /// `LastDamager`, if it is a car alive and active (on a thinking step).
    pub fn damager(&self) -> Option<&Foe> {
        self.damager.and_then(|d| self.foe(d))
    }

    /// `CheckState(SeeTarget | WillSeeTarget)`.
    pub fn can_see_target(&self) -> bool {
        self.sees || self.will_see
    }

    /// `AiBlackboard.Reset`.
    pub fn reset(&mut self) {
        self.sees = false;
        self.will_see = false;
        self.target = None;
    }

    /// Every car gathered that is an enemy within `ScanDistance` on both
    /// axes, alive and active (`GetEnemies`); on a thinking step.
    pub fn enemies(&self) -> impl Iterator<Item = &Foe> {
        let (pos, scan, team) = (self.pos, self.scan_distance, self.team);
        self.seen.iter().map(|s| &s.foe).filter(move |f| f.team != team && closer_by_chebyshev(f.pos, pos, scan))
    }

    fn seen(&self, e: Entity) -> Option<&Seen> {
        self.seen.iter().find(|s| s.foe.entity == e)
    }

    /// A gathered car that is alive and active.
    pub fn foe(&self, e: Entity) -> Option<&Foe> {
        self.seen(e).map(|s| &s.foe)
    }

    /// The target, if it is alive and active.
    pub fn live_target(&self) -> Option<&Foe> {
        self.target.and_then(|t| self.foe(t))
    }

    /// `Target`'s setter: it looks at what it was given at once.
    pub fn set_target(&mut self, target: Entity) {
        self.target = Some(target);
        self.look_at_target();
    }

    /// `UpdateStateAboutTarget`: whether the target is in sight now and
    /// 0.5 s from now; a dead one is dropped.
    fn look_at_target(&mut self) {
        let Some(target) = self.target else {
            return;
        };
        let Some(seen) = self.seen(target) else {
            self.target = None;
            return;
        };
        (self.sees, self.will_see) = (seen.visible, seen.visible_soon);
    }

    /// `AiBlackboard.Update`: the flags start afresh and the target is looked
    /// at again; a damager that is gone, or hurt it 5 s ago, is forgotten.
    fn update(&mut self) {
        self.sees = false;
        self.will_see = false;
        self.look_at_target();
        if let Some(d) = self.damager
            && (self.damager_left <= 0.0 || self.foe(d).is_none())
        {
            self.damager = None;
        }
    }
}
