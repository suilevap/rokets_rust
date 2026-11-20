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
//! The flocking trees (`ZombieFlocking`, `SimpleFlocking`) add the car's
//! friends (`GetFriends`: the cars of its team near it, alive, with what
//! the trees read of them: position, speed, stick, whether the player
//! drives it, and for an AI car what its blackboard says, a [`Mind`]),
//! copied in every step ([`AiBlackboard::gather_mates`]) for a blackboard
//! that watches them; its `Leader` (kept wherever it goes) and `Traits`
//! (ten random numbers of the car's own, drawn when it is made: how fast
//! it flocks, how it weighs the flock's rules, whether it may lead). What
//! a tree does to the world beyond its own car is queued, as the map
//! actions are: an alarm to a friend (`Uii`: [`Alert`], applied by
//! `ai::alert_friends`), an effect ([`Effect`]), and the trigger pulled in a
//! thinking pass ([`AiBlackboard::pull_trigger`]). `Weapon.CanShoot` is
//! [`AiBlackboard::gun_ready`], set by the gather.
//!
//! `LeaderSeeTarget` (the leader, alive, has no wall between it and the
//! target) is looked at with the target, against the floor plan.
//!
//! Not ported: `AiBlackboard`'s other states (`SeeTargetForward`: no tree
//! reads it), its `Orders`,
//! `AdditionalTargetPosition`, `LastDamagerPosition` (copied by `Uii` in C#,
//! read by no tree), `ActionData`
//! (the one slot every action shared: an action keeps its own state), the
//! custom data: the trees so far read none of them (they come with the trees
//! that do). A friend that is deactivated (`IsActive` false) is not gathered
//! (C#'s `GetFriends` did not ask), nor one further than its reach (twice
//! `ScanDistance`, at least 8 and twice the gun's range: the farthest any
//! tree asks), except the leader. `Draw`'s debug overlay (the path and the line to the target). A target that is deactivated (`IsActive` false)
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
/// `AiBlackboard.Traits = new float[10]`.
pub const TRAITS: usize = 10;
/// `GetPredictionPosition`: a prediction in a wall is moved to the nearest
/// of the map's first this many free points (`GetNearestFreePoint(pos, 16)`).
pub const PREDICTION_FREE_POINTS: usize = 16;
/// The least reach of the friends gathered: `Flocking(…, 8.0f)`'s
/// neighbours.
const MIN_FRIEND_REACH: f32 = 8.0;

/// A car of the same team, as the flocking trees see it (`GetFriends`),
/// or the leader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mate {
    pub entity: Entity,
    pub team: u8,
    pub pos: Vec2,
    /// `Speed`, world units a second.
    pub vel: Vec2,
    /// `Control.CurrentAxisControl`, at the start of the step.
    pub stick: Vec2,
    /// `Control.LocalDevice`: the player drives it.
    pub local: bool,
    /// `Control as AIBTBaseControl`: what its blackboard says; `None` for a
    /// car with no behaviour tree.
    pub mind: Option<Mind>,
}

/// What a tree reads of another AI car's blackboard (`Blackboard.Traits`,
/// `Blackboard.Leader`, `Target`, `LastDamager`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mind {
    /// `Traits[0]` (over 0.9: it may lead) and `Traits[1]` (the leaders'
    /// rank, the least first).
    pub traits: [f32; 2],
    /// `Blackboard.Leader`.
    pub leader: Option<Entity>,
    /// `Blackboard.Leader.Control.LocalDevice`.
    pub leader_local: bool,
    /// `Target == null && LastDamager == null`: nobody to fight.
    pub idle: bool,
}

/// `Uii`: a friend is told who hurt this car (`LastDamager`,
/// `LastDamagerTime`), applied after the trees by `ai::alert_friends`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alert {
    pub friend: Entity,
    pub damager: Option<Entity>,
    /// Seconds before the friend forgets the damager.
    pub left: f32,
}

/// An effect a tree asks for, drawn by `ai::draw_effects`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// `SmallTeleportParticle`: a streak at `at` flying along `direction`
    /// (world radians).
    Teleport { at: Vec2, direction: f32 },
}

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
    /// Whether the friends near are gathered every step
    /// ([`gather_mates`](Self::gather_mates)): for the flocking trees.
    pub watch_friends: bool,
    /// `Entity.Weapon.CanShoot`, at the start of the step (the gather sets
    /// it; false with no gun).
    pub gun_ready: bool,
    /// `Traits`: ten random numbers 0 up to 1, the car's own (drawn from a
    /// stream of their own, so the tree's random picks stay as they were).
    pub traits: [f32; TRAITS],
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
    /// `States.LeaderSeeTarget`.
    leader_sees: bool,
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
    /// `Leader`: whom it follows.
    leader: Option<Entity>,
    /// The leader's position when it was last gathered (`SafePosition`).
    leader_at: Vec2,
    /// The friends gathered (and the leader), in the map's order.
    mates: Vec<Mate>,
    /// `Uii`'s alarms, sent by `ai::alert_friends`.
    alerts: Vec<Alert>,
    /// Effects to draw.
    effects: Vec<Effect>,
    /// `Control.Fire = FireType.Shoot` in a thinking pass: `ai::carry_out`
    /// takes it.
    trigger: bool,
}

impl AiBlackboard {
    /// A blackboard with its own random stream, starting from `seed`. The
    /// first think comes at a random moment within the period
    /// (`PeriodicUpdate.Sparse`), so cars are out of step.
    pub fn new(seed: u64) -> AiBlackboard {
        let mut rng = SmallRng::seed_from_u64(seed);
        let think_in = rng.random::<f32>() * THINK_PERIOD;
        let mut traits_rng = SmallRng::seed_from_u64(seed ^ 0x7472_6169_7473);
        AiBlackboard {
            scan_distance: SCAN_DISTANCE,
            fire_accuracy: 1.0,
            watch_friends: false,
            gun_ready: false,
            traits: std::array::from_fn(|_| traits_rng.random()),
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
            leader_sees: false,
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
            leader: None,
            leader_at: Vec2::ZERO,
            mates: Vec::new(),
            alerts: Vec::new(),
            effects: Vec::new(),
            trigger: false,
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

    /// The friends a flocking tree asks about, copied in after
    /// [`gather`](Self::gather): of `cars` (every car alive and active, in the
    /// map's order, `own` being this one), those of its team other than it
    /// within its reach on both axes, and its leader wherever it is.
    pub fn gather_mates(&mut self, own: Entity, cars: &[Mate]) {
        let reach = (self.scan_distance * 2.0)
            .max(MIN_FRIEND_REACH)
            .max(self.gun.map_or(0.0, |g| g.range.range * 2.0));
        let (pos, team, leader) = (self.pos, self.team, self.leader);
        self.mates.clear();
        self.mates.extend(cars.iter().filter(|m| {
            m.entity != own && (Some(m.entity) == leader || (m.team == team && closer_by_chebyshev(m.pos, pos, reach)))
        }));
        if let Some(at) = self.leader().map(|l| l.pos) {
            self.leader_at = at;
        }
    }

    /// `GetFriends(distance)`: the friends gathered within `distance` on
    /// both axes, in the map's order.
    pub fn friends(&self, distance: f32) -> impl Iterator<Item = &Mate> {
        let (pos, team) = (self.pos, self.team);
        self.mates.iter().filter(move |m| m.team == team && closer_by_chebyshev(m.pos, pos, distance))
    }

    /// A gathered friend (or the leader), alive.
    pub fn mate(&self, e: Entity) -> Option<&Mate> {
        self.mates.iter().find(|m| m.entity == e)
    }

    /// `Leader`, whether or not it lives.
    pub fn leader_entity(&self) -> Option<Entity> {
        self.leader
    }

    /// `Leader = …`.
    pub fn set_leader(&mut self, leader: Entity) {
        self.leader = Some(leader);
        if let Some(at) = self.mate(leader).map(|m| m.pos) {
            self.leader_at = at;
        }
    }

    /// The leader, if it is alive (`Leader != null && Leader.Alive`).
    pub fn leader(&self) -> Option<&Mate> {
        self.leader.and_then(|l| self.mate(l))
    }

    /// `Leader != null && Leader.Alive && Leader.Control.LocalDevice`: the
    /// player leads it.
    pub fn player_leads(&self) -> bool {
        self.leader().is_some_and(|l| l.local)
    }

    /// `IsVisible(Leader.Position)`: no wall between the car and its leader
    /// (as last gathered). With no floor plan nothing is in the way.
    pub fn leader_visible(&self) -> bool {
        let at = self.leader().map_or(self.leader_at, |l| l.pos);
        self.terrain.as_ref().is_none_or(|t| !t.line_collides(self.pos, at))
    }

    /// `Leader.GetPredictionPosition(sec)`; where it last was when it is
    /// gone (`SafePosition`).
    pub fn leader_prediction(&self, sec: f32) -> Vec2 {
        match self.leader() {
            Some(l) => self.prediction_position(l.pos, l.vel, sec),
            None => self.leader_at,
        }
    }

    /// `PhysicEntity.GetPredictionPosition`: where something at `pos` going
    /// at `vel` will be in `sec` seconds; a place in a wall (not
    /// `IsPointFree`) is moved to the nearest of the map's first 16 free
    /// points (`GetNearestFreePoint(pos, 16)`, which may be far off).
    pub fn prediction_position(&self, pos: Vec2, vel: Vec2, sec: f32) -> Vec2 {
        let at = pos + vel * sec;
        match &self.terrain {
            Some(t) if !t.is_point_free(at) => t.nearest_free_point(at, PREDICTION_FREE_POINTS),
            _ => at,
        }
    }

    /// `MapManager.GetDistanceFieldValue` and `GetGradientDistancefieldValue`
    /// at a point (0 and zero with no floor plan).
    pub fn room_at(&self, p: Vec2) -> (f32, Vec2) {
        self.terrain.as_ref().map_or((0.0, Vec2::ZERO), |t| (t.distance_at(p), t.gradient_at(p)))
    }

    /// What another tree reads of this blackboard (its leader's
    /// `LocalDevice` is the caller's to fill in).
    pub fn mind(&self) -> Mind {
        Mind {
            traits: [self.traits[0], self.traits[1]],
            leader: self.leader,
            leader_local: false,
            idle: self.live_target().is_none() && self.damager.is_none(),
        }
    }

    /// `LastDamager`, alive or not, until it is forgotten.
    pub fn damager_entity(&self) -> Option<Entity> {
        self.damager
    }

    /// Seconds before the damager is forgotten.
    pub fn damager_left(&self) -> f32 {
        self.damager_left
    }

    /// `Uii` told this car who hurt a friend: taken if it has nobody to
    /// fight (`Target == null && LastDamager == null`). Whether it was.
    pub fn alerted(&mut self, damager: Option<Entity>, left: f32) -> bool {
        if self.live_target().is_some() || self.damager.is_some() {
            return false;
        }
        self.damager = damager;
        self.damager_left = left;
        true
    }

    /// Queues an alarm to a friend (`Uii`).
    pub fn alert(&mut self, alert: Alert) {
        self.alerts.push(alert);
    }

    /// The alarms queued, in order.
    pub fn take_alerts(&mut self) -> Vec<Alert> {
        std::mem::take(&mut self.alerts)
    }

    /// Whether an alarm or an effect waits to be sent.
    pub fn has_news(&self) -> bool {
        !self.alerts.is_empty() || !self.effects.is_empty()
    }

    /// Queues an effect.
    pub fn effect(&mut self, effect: Effect) {
        self.effects.push(effect);
    }

    /// The effects queued, in order.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// `Control.Fire = FireType.Shoot` from a thinking pass.
    pub fn pull_trigger(&mut self) {
        self.trigger = true;
    }

    /// Whether the trigger was pulled in a thinking pass, and forgets it.
    pub fn take_trigger(&mut self) -> bool {
        std::mem::take(&mut self.trigger)
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

    /// `CheckState(LeaderSeeTarget)`: the leader, alive, sees the target.
    pub fn leader_sees_target(&self) -> bool {
        self.leader_sees
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
        self.leader_sees = false;
        self.took_damage = false;
        self.target = None;
        self.path = None;
        self.leader = None;
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
        // `!MapManager.LineCollide(Leader.Position, Target.Position)`.
        self.leader_sees = self
            .leader()
            .is_some_and(|l| self.terrain.as_ref().is_none_or(|t| !t.line_collides(l.pos, at)));
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
        self.leader_sees = false;
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
