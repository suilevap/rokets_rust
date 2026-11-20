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
//! The trees that drive along paths (`ai::trees`: `FollowPathAi`, `ChaserAi`)
//! add what `MoveBase` and its kin read and write: the `Path` to follow (and
//! `PatrolPath`), `TargetPosition`, `LastKnowTargetPosition`,
//! `PathComputeTime`, `PathPeriodControl` with `EntityPreviousPeriodPosition`
//! (the check that a car has not got stuck), `Blocker` and
//! `MapActionOnTirgger`. The navigation graph they ask for paths is the map's
//! ([`Navigation`](super::navigation::Navigation)), handed over by the
//! gather. `MapActionOnTirgger` runs an action (`MapManager.Triggers.RunAction`)
//! and a tree cannot reach the world: it is queued
//! ([`AiBlackboard::run_action`]) and `ai::send_trigger_actions` sends it.
//! What the car's stick was (`Entity.Control.CurrentAxisControl`), its speed,
//! radius and whether it can move (`!IsStatic`) are read off the car with
//! the rest of its [`Body`]. `Target.SafePosition` (the target's position, or
//! where it last was when it is gone) is [`AiBlackboard::target_position_safe`].
//!
//! `TakeDamage` is looked at on each thinking step ([`AiBlackboard::took_damage`]:
//! the car's health and shield together against what they were at the step
//! before). The places the trees pick at random (`GetRandomNodePosition`,
//! `GetRandomFreeVisiblePoint`, `GetRandomFreeNonVisiblePoint`) are the
//! blackboard's methods over the navigation graph and the floor plan
//! ([`Terrain`]), drawing on the car's own random stream.
//!
//! Not ported: `AiBlackboard`'s other states (`SeeTargetForward`,
//! `LeaderSeeTarget` (always off for a turret: no leader)), its
//! `Leader`, `Orders`, `Traits`, `AdditionalTargetPosition`, `ActionData`
//! (the one slot every action shared: an action keeps its own state), the
//! custom data: the trees so far read none of them (they come with the trees
//! that do). `Draw`'s debug overlay (the path and the line to the target). A target that is deactivated (`IsActive` false)
//! counts as gone at once; C# keeps it (alive) as the target until it is out
//! of sight. `GetPredictionPosition`'s correction of a prediction that lands
//! inside a wall (it picks one of the first 16 of the map's free points, an
//! arbitrary place far off): `WillSeeTarget` looks at the plain prediction.

use std::sync::{Arc, Mutex};

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use flatbt_bevy::Tick;

use super::navigation::Navigation;
use crate::map::nav::{Blocker, NavPath};
use crate::map::terrain::Terrain;
use crate::weapon::model::{FireRange, closer_by_chebyshev, due};

/// `AIBTBaseControl._aiUpdateTime`: how often the tree is thought over, s.
pub const THINK_PERIOD: f32 = 0.25;
/// `AIBTBaseControl._scanDistance`: how far a car looks for enemies.
pub const SCAN_DISTANCE: f32 = 16.0;
/// `IsTragetVisible(0.5f)`: `WillSeeTarget` looks this far ahead, s.
pub const WILL_SEE_AHEAD: f32 = 0.5;
/// `AiBlackboard._remeberDamagerTime`, s.
pub const DAMAGER_MEMORY: f32 = 5.0;
/// `AiBlackboard.PathPeriodControl = new PeriodicUpdate(2.0f)`, s.
pub const PATH_PERIOD: f64 = 2.0;

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
    /// `Entity.Weapon.Range` (`None` for a car with no gun): what
    /// `PanicBehaviour` keeps out of.
    pub weapon_range: Option<f32>,
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
    /// `IBaseWeapon.IsImmidiate`: its shot arrives at once (false for the
    /// Plasma gun and the rocket launcher, whose shots fly).
    pub immediate: bool,
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
    /// `Entity.Speed`, world units a second.
    pub velocity: Vec2,
    /// `Entity.Radius`.
    pub radius: f32,
    /// `!Entity.IsStatic`: a car that can move (the turrets that cannot only
    /// turn their guns).
    pub can_move: bool,
    /// `Entity.Control.CurrentAxisControl`: the stick, as the tree last wrote
    /// it.
    pub stick: Vec2,
    /// `Entity.Health`.
    pub health: i32,
    /// `Entity.MaxHealth`.
    pub max_health: i32,
    /// `Entity.Shield.Value`, 0 for a car with no shield.
    pub shield: i32,
}

/// `PathPeriodControl` and `EntityPreviousPeriodPosition`: every two seconds
/// a car on an open path is checked for having moved. The check is made while
/// asking whether the action is still in progress, which sees the blackboard
/// only by reference, so these are behind a lock.
#[derive(Debug, Default)]
struct PathWatch(Mutex<Watch>);

#[derive(Clone, Copy, Debug, Default)]
struct Watch {
    /// `PeriodicUpdate._invalidatedTime`, s.
    until: f64,
    /// `EntityPreviousPeriodPosition`.
    previous: Vec2,
}

impl Clone for PathWatch {
    fn clone(&self) -> PathWatch {
        PathWatch(Mutex::new(self.state()))
    }
}

impl PathWatch {
    fn state(&self) -> Watch {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn with<R>(&self, f: impl FnOnce(&mut Watch) -> R) -> R {
        f(&mut self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

/// `AiBlackboard`, one per AI car.
#[derive(Component, Debug, Clone)]
pub struct AiBlackboard {
    /// `ScanDistance`: how far it looks for enemies.
    pub scan_distance: f32,
    /// `FireAccuracy`: 1 fires at what is in the line, more at what is near
    /// it.
    pub fire_accuracy: f32,
    /// Whether the enemies within `ScanDistance` are gathered every step, not
    /// only when the tree thinks: for a tree whose action asks `GetEnemies`
    /// while it ticks (`TryFireToAnyone`).
    pub watch_enemies: bool,
    /// The car's position (`Entity.Position`), world units, y up.
    pub pos: Vec2,
    /// The car's heading (`Entity.Direction`), world radians.
    pub heading: f32,
    /// `Entity.Control.Team`.
    pub team: u8,
    /// `Entity.Weapon`; `None` for a car with no gun.
    pub gun: Option<Gun>,
    /// `Entity.Speed`.
    pub velocity: Vec2,
    /// `Entity.Radius`.
    pub radius: f32,
    /// `!Entity.IsStatic`.
    pub can_move: bool,
    /// `Entity.Control.CurrentAxisControl`, as it was at the start of the
    /// step.
    pub stick: Vec2,
    /// `Time.ElapsedGameTime`, s.
    pub step: f32,
    /// The map's navigation graph (`MapManager.Navigation`), once the gather
    /// has handed it over.
    pub nav: Option<Navigation>,
    /// The map's floor plan (`MapManager`'s distance field and free points),
    /// handed over by the gather.
    pub terrain: Option<Arc<Terrain>>,
    /// `Entity.Health`.
    pub health: i32,
    /// `Entity.MaxHealth`.
    pub max_health: i32,
    /// `Path`: the way `MoveBase` follows.
    pub path: Option<NavPath>,
    /// `PatrolPath`: where a patrolling tree goes (set by `SetPath`; no tree
    /// reads it yet).
    pub patrol_path: Option<NavPath>,
    /// `TargetPosition`: where the car is going (a point of the map, world).
    pub target_position: Vec2,
    /// `LastKnowTargetPosition`: where the target was when it was last seen.
    pub last_known_target_position: Vec2,
    /// `PathComputeTime`: when the path was made, s of game time.
    pub path_compute_time: f64,
    /// `Blocker`: what closed the way the last path found.
    pub blocker: Option<Blocker>,
    /// `MapActionOnTirgger`: the map action `TriggerMapAction` runs.
    pub map_action_on_trigger: Option<String>,
    /// Map actions the tree asked for (`MapManager.Triggers.RunAction`),
    /// sent by `ai::send_trigger_actions`.
    actions: Vec<String>,
    /// A tree ended an action that cleared the stick
    /// (`Control.ClearAxis`): `ai::carry_out` takes it.
    stick_cleared: bool,
    /// `PathPeriodControl`, `EntityPreviousPeriodPosition`.
    watch: PathWatch,
    /// `Target.SafePosition`: where the target is, or last was.
    target_at: Vec2,
    /// The cars the tree can ask about: the target, and on a thinking step
    /// the damager and the enemies within `ScanDistance`.
    seen: Vec<Seen>,
    /// `Target`: who it fights, or looked at last.
    target: Option<Entity>,
    /// `States.SeeTarget`, as of the last [`update`](Self::update).
    sees: bool,
    /// `States.WillSeeTarget`.
    will_see: bool,
    /// `States.TakeDamage`: at the last [`update`](Self::update) the car's
    /// health and shield together were less than at the one before.
    took_damage: bool,
    /// `_previousTotalHealth`; before the first look, the car's health.
    previous_total_health: Option<i32>,
    /// `Entity.Health + Entity.Shield.Value`.
    total_health: i32,
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
            watch_enemies: false,
            pos: Vec2::ZERO,
            heading: 0.0,
            team: 0,
            gun: None,
            velocity: Vec2::ZERO,
            radius: 0.0,
            can_move: true,
            stick: Vec2::ZERO,
            step: 0.0,
            nav: None,
            terrain: None,
            health: 0,
            max_health: 0,
            path: None,
            patrol_path: None,
            target_position: Vec2::ZERO,
            last_known_target_position: Vec2::ZERO,
            path_compute_time: 0.0,
            blocker: None,
            map_action_on_trigger: None,
            actions: Vec::new(),
            stick_cleared: false,
            watch: PathWatch::default(),
            target_at: Vec2::ZERO,
            seen: Vec::new(),
            target: None,
            sees: false,
            will_see: false,
            took_damage: false,
            previous_total_health: None,
            total_health: 0,
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
        self.step = dt;
        self.damager_left -= dt;
        self.pos = body.pos;
        self.heading = body.heading;
        self.team = body.team;
        self.gun = body.gun;
        self.velocity = body.velocity;
        self.radius = body.radius;
        self.can_move = body.can_move;
        self.stick = body.stick;
        self.health = body.health;
        self.max_health = body.max_health;
        // `_previousTotalHealth = Entity.Health` when the control is made.
        self.previous_total_health.get_or_insert(body.health);
        self.total_health = body.health + body.shield;
        self.think_in -= dt;
        let forced = due(self.think_in);
        if forced {
            self.think_in = THINK_PERIOD;
        }
        let think = forced || !running;
        self.tick = if think { Tick::Evaluate } else { Tick::Resume };
        let (pos, scan, team, target, damager) = (self.pos, self.scan_distance, self.team, self.target, self.damager);
        let watch = think || self.watch_enemies;
        let wanted = |f: &&Foe| {
            Some(f.entity) == target
                || (think && Some(f.entity) == damager)
                || (watch && f.team != team && closer_by_chebyshev(f.pos, pos, scan))
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
        if let Some(at) = self.target.and_then(|t| self.seen(t)).map(|s| s.foe.pos) {
            self.target_at = at;
        }
        if think {
            self.update();
        }
    }

    /// `Time.TotalGameTime`, s.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// `AIBTBaseControl.Sleep(sec)`: the next forced think is `sec` seconds
    /// (and a period) away.
    pub fn sleep(&mut self, sec: f32) {
        self.think_in = sec + THINK_PERIOD;
    }

    /// `PathPeriodControl.Update()`.
    pub fn path_period_update(&self) {
        let until = self.now + PATH_PERIOD;
        self.watch.with(|w| w.until = until);
    }

    /// `PathPeriodControl.Avalaible()`: the period is over.
    pub fn path_period_available(&self) -> bool {
        self.now > self.watch.state().until
    }

    /// `EntityPreviousPeriodPosition`.
    pub fn previous_period_position(&self) -> Vec2 {
        self.watch.state().previous
    }

    /// `EntityPreviousPeriodPosition = Entity.Position`.
    pub fn remember_period_position(&self) {
        let pos = self.pos;
        self.watch.with(|w| w.previous = pos);
    }

    /// `Control.ClearAxis()`, asked of the car after the tree has run.
    pub fn clear_stick(&mut self) {
        self.stick = Vec2::ZERO;
        self.stick_cleared = true;
    }

    /// Whether the stick is to be cleared, and forgets it.
    pub fn take_stick_cleared(&mut self) -> bool {
        std::mem::take(&mut self.stick_cleared)
    }

    /// `MapManager.Triggers.RunAction(name, Entity.CustomId)`, to be sent
    /// once the tree is done.
    pub fn run_action(&mut self, name: String) {
        self.actions.push(name);
    }

    /// Whether a map action waits to be sent.
    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }

    /// The map actions the tree asked for, in order.
    pub fn take_actions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.actions)
    }

    /// `Target.SafePosition`: where the target is now, or where it was when
    /// it was last gathered (the target dead or gone).
    pub fn target_position_safe(&self) -> Vec2 {
        self.target_at
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

    /// `CheckState(SeeTarget)`: in sight now.
    pub fn sees_target(&self) -> bool {
        self.sees
    }

    /// `CheckState(TakeDamage)`: as of the last time the tree thought, the
    /// car had lost health or shield since the time before.
    pub fn took_damage(&self) -> bool {
        self.took_damage
    }

    /// `MapManager.GetRandomNodePosition(pos, distance)`: the position of
    /// one of the navigation nodes within `distance` of `pos` on both axes
    /// (`RandomHelper.NextIntFakeRange(0, count - 1)`: never the last of
    /// them, unless it is the only one); `pos` itself when there are none or
    /// the map has no graph.
    pub fn random_node_position(&mut self, pos: Vec2, distance: f32) -> Vec2 {
        let Some(nav) = self.nav.clone() else {
            return pos;
        };
        let rand = self.random();
        nav.read(|graph| {
            let nodes: Vec<Vec2> = graph.get_nodes(pos, distance).map(|n| n.pos).collect();
            match nodes.len() {
                0 => pos,
                count => nodes[((count - 1) as f32 * rand) as usize],
            }
        })
    }

    /// `MapManager.GetRandomFreeVisiblePoint(pos, maxDistance)`; the origin
    /// (no point) with no floor plan.
    pub fn random_free_visible_point(&mut self, pos: Vec2, max_distance: f32) -> Vec2 {
        let Some(terrain) = self.terrain.clone() else {
            return Vec2::ZERO;
        };
        terrain.random_free_visible_point(pos, 0.0, max_distance, &mut || self.rng.random())
    }

    /// `MapManager.GetRandomFreeNonVisiblePoint(pos, from, maxDistance)`; the
    /// origin (no point) with no floor plan.
    pub fn random_free_non_visible_point(&mut self, pos: Vec2, from: Vec2, max_distance: f32) -> Vec2 {
        let Some(terrain) = self.terrain.clone() else {
            return Vec2::ZERO;
        };
        terrain.random_free_non_visible_point(pos, from, max_distance, &mut || self.rng.random())
    }

    /// `AiBlackboard.Reset`.
    pub fn reset(&mut self) {
        self.sees = false;
        self.will_see = false;
        self.took_damage = false;
        self.target = None;
        self.path = None;
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
        if let Some(at) = self.seen(target).map(|s| s.foe.pos) {
            self.target_at = at;
        }
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
        let (visible, soon, at) = (seen.visible, seen.visible_soon, seen.foe.pos);
        (self.sees, self.will_see) = (visible, soon);
        if visible {
            self.last_known_target_position = at;
        }
    }

    /// `AiBlackboard.Update`: the flags start afresh and the target is looked
    /// at again; the car's health and shield are compared with the last time
    /// (`TakeDamage`); a damager that is gone, or hurt it 5 s ago, is
    /// forgotten.
    fn update(&mut self) {
        self.sees = false;
        self.will_see = false;
        self.took_damage = false;
        self.look_at_target();
        self.took_damage = self.previous_total_health.is_some_and(|before| self.total_health < before);
        self.previous_total_health = Some(self.total_health);
        if let Some(d) = self.damager
            && (self.damager_left <= 0.0 || self.foe(d).is_none())
        {
            self.damager = None;
        }
    }
}
