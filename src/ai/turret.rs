//! The turret's mind: `BTActionFactory.TurretAi`, flattened.
//!
//! C# origin: `AiControlFactory` makes `AiSimpleTurret` (and `…Friend`)
//! an `AIBTBaseControl` running the behaviour tree
//! `Selector(SimpleAttack, SimpleScanning)`; `AITurret` (a hand-written
//! turret that came before it) is commented out there. The tree, with its
//! `BTLib` running rules, is what this module reproduces as a small state
//! machine; the generic tree engine, the blackboard and the other trees are
//! the behaviour-tree slice.
//!
//! What the tree does, and how it is run (`AIBTBaseControl.UpdateControl`,
//! `BT.Context.Update`):
//!
//! - Every step the running action is ticked: the attack aims at the
//!   target (its position 0.2 s ahead, `GetSimplePredictionPosition`),
//!   pulls the trigger when the target is within the weapon's fire range
//!   *as the gun points now* (`TargetInFireRange`, with the radius scaled by
//!   `FireAccuracy`: more than 1 fires at things further off the line), and
//!   steers the car at the target (`SimpleFollowByTarget`: a car that can
//!   move chases, a static one only turns its gun, as the aim is the same).
//! - The tree is thought over again every 0.25 s (a `Sparse` timer, so cars
//!   are out of step), at once when the action ends, and every step while
//!   nothing runs. Thinking is: the blackboard looks again (whether the
//!   target is alive, `SeeTarget`, `WillSeeTarget`, forgetting the last
//!   damager after 5 s), then the tree is walked from the top. A running
//!   attack is not left for a nearer enemy (the sequence resumes its running
//!   child without asking `FindNearestEnemy` again); scanning is dropped for
//!   an attack at once.
//! - `FindNearestEnemy`: whoever last hurt it (of another team, whatever the
//!   distance or walls), else the current target while it stays within
//!   `ScanDistance` (16 units unless `AIAdjustment` or the control sets
//!   it) and is in sight (or will be 0.5 s from now), else the nearest by
//!   `|dx| + |dy|` of those within `ScanDistance` on both axes, other team,
//!   alive, active. If the current target has gone out of sight but is in
//!   range, any *other* enemy is chosen, and when there is none it finds
//!   nothing and scans (it keeps the old target, which it looks at again
//!   next time).
//! - The attack starts only when the target is in sight now or in 0.5 s
//!   (`SeeTarget | WillSeeTarget`, `MapManager.LineCollide`), and goes on
//!   while that holds and the target lives; then the turret scans.
//! - Scanning: stand 1 s (the gun stays where it was), then pick a random
//!   direction within 110 % of the gun's `MaxAngle` of the car's heading
//!   (a gun with no angle: any direction) and turn the gun there for 1 s;
//!   then the tree ends and is started over.
//!
//! The control writes only what the tree writes: the aim and the stick
//! stay as they were between writes (the stick of a car that can move keeps
//! its last direction after the target is lost, as in C#), and the trigger
//! is pulled for the step it was ticked.
//!
//! Not ported (here, the rest is in [`super`]):
//!
//! - The tree engine itself (`BTLib`, `AiBlackboard`) and the trees but
//!   this one; `TurretAiFireToAnyone` (`AiSimpleTurretFriendFireAvailableTarget`,
//!   `SimpleAttackByAngle`, `FindAngleNearestEnemy`, `TryFireToAnyone`), which no
//!   map uses.
//! - `SimpleAttackTarget`'s second branch (`FireToTarget` alone): it starts
//!   exactly when the first does and is left under the same conditions, so
//!   it can never run beside it. A turret with no weapon never attacks (the
//!   tree starts an attack that is at once over, and fails), as in C#.
//! - `GetPredictionPosition`'s correction of a prediction that lands inside
//!   a wall (it picks one of the first 16 of the map's free points, an
//!   arbitrary place far off): the prediction is used as it is.
//! - `AiBlackboard`'s other states (`SeeTargetForward`, `LeaderSeeTarget`,
//!   `TakeDamage`), its leader, orders, paths and traits: this tree never
//!   reads them (`LeaderSeeTarget` is always off for a turret). `ActionOnBTTrigger`
//!   (read by `TriggerMapAction` only).
//! - A target that is deactivated (`IsActive` false) counts as gone at once;
//!   C# keeps it (alive) as the target until it is out of sight.
//! - C# draws `DEBUG && AI` overlays of the control's state.

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::weapon::model::{closer_by_chebyshev, due};

/// `AIBTBaseControl._aiUpdateTime`.
pub const THINK_PERIOD: f32 = 0.25;
/// `AIBTBaseControl._scanDistance`.
pub const SCAN_DISTANCE: f32 = 16.0;
/// `SimpleAttackTarget(0.2f)`: how far ahead of the target it aims, s.
pub const AIM_AHEAD: f32 = 0.2;
/// `IsTragetVisible(0.5f)`: `WillSeeTarget` looks this far ahead, s.
pub const WILL_SEE_AHEAD: f32 = 0.5;
/// `SimpleScanning`: `Timer(1.0f, …)` twice, s.
pub const IDLE_TIME: f32 = 1.0;
pub const SCAN_TIME: f32 = 1.0;
/// `SimpleScanning`: the sweep is ±1.1 × the gun's `MaxAngle`.
pub const SCAN_SWEEP: f32 = 1.1;
/// `AiBlackboard._remeberDamagerTime`, s.
pub const DAMAGER_MEMORY: f32 = 5.0;

/// A car the turret may look at: alive and active, with what it needs to
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

/// The turret's gun, as the turret needs it.
#[derive(Clone, Copy)]
pub struct Gun<'a> {
    /// A gun that aims (`IBaseComplexWeapon`), not the Hammer.
    pub complex: bool,
    /// `MaxAngle`, after `AdjustWeaponMaxAngle`.
    pub max_angle: f32,
    /// `TargetInFireRange(position, radius)`.
    pub in_range: &'a dyn Fn(Vec2, f32) -> bool,
}

/// What the turret sees this step.
pub struct Scene<'a> {
    pub pos: Vec2,
    /// The car's heading, world radians.
    pub heading: f32,
    pub team: u8,
    pub gun: Option<Gun<'a>>,
    /// Every car alive and active (this one too: it is no enemy of itself).
    pub cars: &'a [Foe],
    /// Whether a wall is in the way (`MapManager.LineCollide`).
    pub blocked: &'a dyn Fn(Vec2, Vec2) -> bool,
}

impl Scene<'_> {
    fn foe(&self, e: Entity) -> Option<&Foe> {
        self.cars.iter().find(|f| f.entity == e)
    }
}

/// What the control writes this step; `None` leaves the intent as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orders {
    /// `CurrentAxisControl`.
    pub move_dir: Option<Vec2>,
    /// `WeaponAxisControl`.
    pub aim_dir: Option<Vec2>,
    /// `Fire |= FireType.Shoot` this step.
    pub fire: bool,
}

/// Which node of the tree is running (`Context.LastRunningNode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing: the tree ended or has not started.
    Done,
    /// `All(FireToTarget, SimpleFollowByTarget)`.
    Attack,
    /// `Timer(1, Idle)`.
    Idle,
    /// `Timer(1, Scanning)`.
    Scanning,
}

/// How an attempt at the attack branch came out (`BT.Status`).
enum Status {
    Running,
    Ok,
    Fail,
}

/// An enemy turret's or friendly turret's control (`AIBTBaseControl` with
/// `TurretAi`).
#[derive(Component, Debug, Clone)]
pub struct Turret {
    /// `ScanDistance`: how far it looks for enemies.
    pub scan_distance: f32,
    /// `FireAccuracy`: 1 fires at what is in the line, more at what is
    /// near it.
    pub fire_accuracy: f32,
    think_in: f32,
    phase: Phase,
    /// Seconds since the blackboard's one timer was started (`StartTimer`).
    age: f32,
    target: Option<Entity>,
    sees: bool,
    will_see: bool,
    damager: Option<Entity>,
    damager_left: f32,
    /// `TargetPosition` of the scan, world.
    scan_at: Vec2,
    rng: SmallRng,
}

impl Turret {
    /// A turret with its own random stream; the first think comes at a
    /// random moment within the period (`PeriodicUpdate.Sparse`).
    pub fn new(seed: u64) -> Turret {
        let mut rng = SmallRng::seed_from_u64(seed);
        Turret {
            scan_distance: SCAN_DISTANCE,
            fire_accuracy: 1.0,
            think_in: rng.random::<f32>() * THINK_PERIOD,
            phase: Phase::Done,
            age: 0.0,
            target: None,
            sees: false,
            will_see: false,
            damager: None,
            damager_left: 0.0,
            scan_at: Vec2::ZERO,
            rng,
        }
    }

    /// Which node is running.
    pub fn phase(&self) -> Phase {
        self.phase
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

    /// `AIBTBaseControl.Reset`.
    pub fn reset(&mut self) {
        self.phase = Phase::Done;
        self.target = None;
        self.sees = false;
        self.will_see = false;
    }

    /// One step of `UpdateControl`: tick what runs, think when it is time.
    pub fn step(&mut self, dt: f32, scene: &Scene) -> Orders {
        let mut orders = Orders::default();
        self.age += dt;
        self.think_in -= dt;
        self.damager_left -= dt;
        let forced = due(self.think_in);
        if forced {
            self.think_in = THINK_PERIOD;
        }
        let mut think = forced;
        match self.phase {
            Phase::Done => think = true,
            Phase::Attack => {
                if self.attack_in_progress(scene) {
                    self.tick_attack(scene, &mut orders);
                } else {
                    think = true;
                }
            }
            Phase::Idle => {}
            Phase::Scanning => orders.aim_dir = Some(self.scan_aim(scene)),
        }
        if think {
            self.think(scene);
        }
        orders
    }

    /// `FireToTarget.IsInProgress` and `SimpleFollowByTarget.IsInProgress`.
    fn attack_in_progress(&self, scene: &Scene) -> bool {
        scene.gun.is_some() && self.live_target(scene).is_some() && (self.sees || self.will_see)
    }

    fn live_target<'a>(&self, scene: &'a Scene) -> Option<&'a Foe> {
        self.target.and_then(|t| scene.foe(t))
    }

    /// `FireToTarget.Tick` then `SimpleFollowByTarget.Tick` (the latter
    /// writes the aim last, to the same direction).
    fn tick_attack(&self, scene: &Scene, orders: &mut Orders) {
        let (Some(foe), Some(gun)) = (self.live_target(scene), scene.gun) else {
            return;
        };
        let ahead = foe.pos + foe.vel * AIM_AHEAD;
        let direction = (ahead - scene.pos).normalize_or_zero();
        orders.aim_dir = Some(direction);
        orders.move_dir = Some(direction);
        orders.fire = (gun.in_range)(foe.pos, foe.radius * self.fire_accuracy);
    }

    /// `AiBlackboard.Update` and `Context.Think`: the tree from the root.
    fn think(&mut self, scene: &Scene) {
        self.sees = false;
        self.will_see = false;
        self.look_at_target(scene);
        if let Some(d) = self.damager
            && (self.damager_left <= 0.0 || scene.foe(d).is_none())
        {
            self.damager = None;
        }
        match self.attack(scene) {
            Status::Running => self.phase = Phase::Attack,
            Status::Ok => self.phase = Phase::Done,
            Status::Fail => self.scan(scene),
        }
    }

    /// `UpdateStateAboutTarget`: whether the target is in sight now and
    /// 0.5 s from now; a dead one is dropped.
    fn look_at_target(&mut self, scene: &Scene) {
        let Some(target) = self.target else {
            return;
        };
        let Some(foe) = scene.foe(target) else {
            self.target = None;
            return;
        };
        self.sees = !(scene.blocked)(scene.pos, foe.pos);
        self.will_see = !(scene.blocked)(scene.pos, foe.pos + foe.vel * WILL_SEE_AHEAD);
    }

    /// `SimpleAttack`: `FindNearestEnemy`, then `SimpleAttackTarget`.
    fn attack(&mut self, scene: &Scene) -> Status {
        if self.phase == Phase::Attack {
            // The sequence resumes its running child.
            return if self.attack_in_progress(scene) {
                Status::Running
            } else if self.live_target(scene).is_none() {
                // `FireToTarget.Complete`: the target is gone.
                Status::Ok
            } else {
                Status::Fail
            };
        }
        if !self.find_nearest_enemy(scene) {
            return Status::Fail;
        }
        let starts = self.live_target(scene).is_some() && (self.sees || self.will_see);
        if starts && self.attack_in_progress(scene) {
            Status::Running
        } else {
            Status::Fail
        }
    }

    /// `FindNearestEnemy.Start`: sets the target when there is one.
    fn find_nearest_enemy(&mut self, scene: &Scene) -> bool {
        let hit_back = self
            .damager
            .filter(|&d| scene.foe(d).is_some_and(|f| f.team != scene.team));
        let found = hit_back.or_else(|| self.nearest_enemy(scene));
        if let Some(target) = found {
            // `Target`'s setter looks at it.
            self.target = Some(target);
            self.look_at_target(scene);
        }
        found.is_some()
    }

    /// `FindNearestEnemy.GetNearestEnemy`.
    fn nearest_enemy(&self, scene: &Scene) -> Option<Entity> {
        let scan = self.scan_distance;
        let enemy = |f: &&Foe| f.team != scene.team && closer_by_chebyshev(f.pos, scene.pos, scan);
        let best = |skip: Option<Entity>| {
            // `EntitiesHelper.GetBest`: the least `|dx| + |dy|`, the first
            // of equals.
            let mut best: Option<(f32, Entity)> = None;
            for f in scene.cars.iter().filter(enemy).filter(|f| Some(f.entity) != skip) {
                let d = (f.pos - scene.pos).abs();
                let manhattan = d.x + d.y;
                if manhattan < best.map_or(f32::MAX, |b| b.0) {
                    best = Some((manhattan, f.entity));
                }
            }
            best.map(|b| b.1)
        };
        match self.live_target(scene) {
            Some(last) if last.team != scene.team && last.pos.distance_squared(scene.pos) < scan * scan => {
                if self.sees || self.will_see {
                    Some(last.entity)
                } else {
                    best(Some(last.entity))
                }
            }
            _ => best(None),
        }
    }

    /// `SimpleScanning`: `Sequence(Timer(1, Idle), Timer(1, Scanning))`.
    fn scan(&mut self, scene: &Scene) {
        // Under a second the timer is still running (a step's spare, as
        // C#'s tick count had).
        let running = |age: f32, time: f32| age < time - 1e-6;
        match self.phase {
            Phase::Idle if running(self.age, IDLE_TIME) => {}
            Phase::Idle => {
                self.age = 0.0;
                self.start_scanning(scene);
                self.phase = Phase::Scanning;
            }
            Phase::Scanning if running(self.age, SCAN_TIME) => {}
            Phase::Scanning => self.phase = Phase::Done,
            _ => {
                self.age = 0.0;
                self.phase = Phase::Idle;
            }
        }
    }

    /// `Scanning`'s start: where to look.
    fn start_scanning(&mut self, scene: &Scene) {
        let direction = match scene.gun {
            Some(gun) if gun.complex => {
                let turn = (self.rng.random::<f32>() * 2.0 - 1.0) * SCAN_SWEEP * gun.max_angle;
                Vec2::from_angle(scene.heading + turn)
            }
            _ => Vec2::new(self.rng.random::<f32>() * 2.0 - 1.0, self.rng.random::<f32>() * 2.0 - 1.0),
        };
        self.scan_at = scene.pos + direction * self.scan_distance;
    }

    /// `Scanning`'s tick: `ToGoWorld(TargetPosition, ScanDistance)`.
    fn scan_aim(&self, scene: &Scene) -> Vec2 {
        let to = self.scan_at - scene.pos;
        if to.length_squared() < self.scan_distance * self.scan_distance {
            to / self.scan_distance
        } else {
            to.normalize_or_zero()
        }
    }
}
