//! `ZombieFlocking` (`ZombieFlocking`, `ZombieFlockingFriend`) and its nodes
//! (`Flocking`, `FindNearestLeader`, the super attack, `Uii`), run as the
//! plugin runs them (gather, friends, tick, the act on the car) on a few
//! cars that are points moving as their sticks say, over a map's floor plan.

use std::sync::Arc;

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt_bevy::prelude::*;
use tilerider::ai::trees::{ZOMBIE_SCAN_DISTANCE, zombie_flocking};
use tilerider::ai::{AiBlackboard, Alert, Body, Effect, Foe, Gun, Mate, Mind, Navigation, Orders, tick_mode};
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::Terrain;
use tilerider::weapon::model::{FireRange, ZOMBIE};

const DT: f32 = 1.0 / 60.0;
/// How fast a full stick drives a point car, units a second.
const SPEED: f32 = 4.0;
const PLAYER: u32 = 50;

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

/// A room `w` × `h` cells inside a ring of wall, a cell a unit.
fn room(w: usize, h: usize) -> MapGrid {
    let mut rows = String::from("\n");
    for y in 0..h + 2 {
        for x in 0..w + 2 {
            rows.push(if x == 0 || y == 0 || x == w + 1 || y == h + 1 { '*' } else { '.' });
        }
        rows.push('\n');
    }
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

/// A cell's middle in the world (y up).
fn at(x: f32, y: f32) -> Vec2 {
    Vec2::new(x + 0.5, -(y + 0.5))
}

/// A zombie and its mind.
struct Zed<B: TreeBuilder<AiBlackboard, Orders>> {
    entity: Entity,
    team: u8,
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    stick: Vec2,
    health: i32,
    /// `Weapon.CanShoot`.
    ready: bool,
    /// What it did each step: orders, and whether it pulled the trigger.
    orders: Vec<Orders>,
    pulls: usize,
}

struct Sim<B: TreeBuilder<AiBlackboard, Orders>> {
    grid: MapGrid,
    nav: Navigation,
    terrain: Arc<Terrain>,
    builder: B,
    zeds: Vec<Zed<B>>,
    /// The player's car (team 0) and how it moves, if any.
    player: Option<(Vec2, Vec2)>,
    /// Cars of another team, standing.
    enemies: Vec<Foe>,
}

impl<B: TreeBuilder<AiBlackboard, Orders> + Copy> Sim<B> {
    fn new(builder: B, w: usize, h: usize) -> Self {
        let grid = room(w, h);
        let nav = Navigation::build(&grid, tilerider::units::CellSize(1.0));
        let terrain = Arc::new(Terrain::from_grid(&grid, 1.0));
        Sim { grid, nav, terrain, builder, zeds: Vec::new(), player: None, enemies: Vec::new() }
    }

    /// A zombie of `team` (0: `ZombieFlockingFriend`) at `pos`.
    fn zombie(&mut self, n: u32, team: u8, pos: Vec2) -> usize {
        let mut bb = AiBlackboard::new(n as u64);
        bb.scan_distance = ZOMBIE_SCAN_DISTANCE;
        bb.watch_friends = true;
        bb.nav = Some(self.nav.clone());
        bb.terrain = Some(self.terrain.clone());
        self.zeds.push(Zed {
            entity: id(n),
            team,
            bb,
            behavior: Behavior::for_tree(self.builder),
            tree: BehaviorTree::new(&self.builder, tick_mode),
            act: None,
            pos,
            stick: Vec2::ZERO,
            health: 16,
            ready: false,
            orders: Vec::new(),
            pulls: 0,
        });
        self.zeds.len() - 1
    }

    fn step(&mut self) {
        let mut foes: Vec<Foe> = self
            .zeds
            .iter()
            .filter(|z| z.health > 0)
            .map(|z| Foe { entity: z.entity, team: z.team, pos: z.pos, vel: z.stick * SPEED, radius: 0.98, weapon_range: Some(ZOMBIE.range) })
            .collect();
        let mut mates: Vec<Mate> = self
            .zeds
            .iter()
            .filter(|z| z.health > 0)
            .map(|z| {
                let mind = z.bb.mind();
                let leader_local = mind.leader == Some(id(PLAYER));
                Mate {
                    entity: z.entity,
                    team: z.team,
                    pos: z.pos,
                    vel: z.stick * SPEED,
                    stick: z.stick,
                    local: false,
                    mind: Some(Mind { leader_local, ..mind }),
                }
            })
            .collect();
        if let Some((pos, vel)) = self.player {
            foes.push(Foe { entity: id(PLAYER), team: 0, pos, vel, radius: 0.98, weapon_range: Some(ZOMBIE.range) });
            mates.push(Mate { entity: id(PLAYER), team: 0, pos, vel, stick: vel / SPEED, local: true, mind: None });
        }
        for e in &self.enemies {
            foes.push(*e);
            mates.push(Mate { entity: e.entity, team: e.team, pos: e.pos, vel: e.vel, stick: Vec2::ZERO, local: false, mind: None });
        }
        let grid = &self.grid;
        let blocked = |a: Vec2, b: Vec2| grid.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let mut alerts: Vec<Alert> = Vec::new();
        for z in &mut self.zeds {
            if z.health <= 0 {
                continue;
            }
            let body = Body {
                pos: z.pos,
                heading: 0.0,
                team: z.team,
                gun: Some(Gun {
                    complex: false,
                    max_angle: 0.0,
                    range: FireRange { from: z.pos, direction: 0.0, range: ZOMBIE.range, dispersion: 0.0 },
                    immediate: true,
                }),
                velocity: z.stick * SPEED,
                radius: 0.98,
                can_move: true,
                stick: z.stick,
                health: z.health,
                max_health: 16,
                shield: 0,
            };
            z.bb.gather(DT, z.act.is_some(), body, &foes, &blocked);
            z.bb.gun_ready = z.ready;
            z.bb.gather_mates(z.entity, &mates);
            let mode = z.bb.tick.entry_mode().expect("gathered");
            z.act = z.behavior.tick(z.tree.get(), &mut z.bb, mode);
            let orders = z.act.unwrap_or_default();
            if z.bb.take_stick_cleared() {
                z.stick = Vec2::ZERO;
            }
            if let Some(m) = orders.move_dir {
                z.stick = m;
            }
            if z.bb.take_trigger() {
                z.pulls += 1;
                z.ready = false;
            }
            alerts.extend(z.bb.take_alerts());
            let step = z.stick * SPEED * DT;
            for axis in [Vec2::X, Vec2::Y] {
                let to = z.pos + step * axis;
                if !grid.is_wall(to.x.floor() as i32, (-to.y).floor() as i32) {
                    z.pos = to;
                }
            }
            z.orders.push(orders);
        }
        for alert in alerts {
            if let Some(z) = self.zeds.iter_mut().find(|z| z.entity == alert.friend) {
                z.bb.alerted(alert.damager, alert.left);
            }
        }
        if let Some((pos, vel)) = self.player.as_mut() {
            *pos += *vel * DT;
        }
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }

    fn path(&self, i: usize) -> String {
        self.zeds[i].behavior.describe(self.zeds[i].tree.get()).to_string()
    }
}

/// A car of the player's team, for the enemy zombies (team 1).
fn enemy(n: u32, pos: Vec2) -> Foe {
    Foe { entity: id(n), team: 0, pos, vel: Vec2::ZERO, radius: 0.98, weapon_range: Some(ZOMBIE.range) }
}

#[test]
fn friendly_zombies_take_the_player_as_leader_run_the_trigger_once_and_follow() {
    let mut sim = Sim::new(zombie_flocking, 60, 12);
    for (n, (x, y)) in [(1, (6.0, 4.0)), (2, (6.0, 7.0)), (3, (8.0, 5.0)), (4, (7.0, 8.0))] {
        let i = sim.zombie(n, 0, at(x, y));
        sim.zeds[i].bb.map_action_on_trigger = Some("seqZombieConnected".into());
    }
    sim.player = Some((at(4.0, 6.0), Vec2::ZERO));
    sim.run(1.0);
    for z in &mut sim.zeds {
        assert_eq!(z.bb.leader_entity(), Some(id(PLAYER)));
        // `ActionOnBTTrigger` is run once, when the player first leads.
        assert_eq!(z.bb.take_actions(), vec!["seqZombieConnected".to_string()]);
        assert_eq!(z.bb.map_action_on_trigger, None);
    }
    // The player drives off along the room; the flock comes after where it
    // will be in 4 s, holding together.
    sim.player = Some((at(4.0, 6.0), Vec2::new(2.0, 0.0)));
    sim.run(12.0);
    let player = sim.player.unwrap().0;
    for (i, z) in sim.zeds.iter().enumerate() {
        assert!(z.pos.x > at(30.0, 0.0).x, "zombie {i} at {} (player at {player}): {}", z.pos, sim.path(i));
        assert!(z.pos.distance(player) < 14.0, "zombie {i} at {} kept up with {player}", z.pos);
        assert_eq!(z.bb.leader_entity(), Some(id(PLAYER)));
    }
    for z in &mut sim.zeds {
        assert!(z.bb.take_actions().is_empty(), "the trigger is run once");
    }
    // Nobody stands on another.
    for a in 0..sim.zeds.len() {
        for b in a + 1..sim.zeds.len() {
            assert!(sim.zeds[a].pos.distance(sim.zeds[b].pos) > 0.3);
        }
    }
}

#[test]
fn the_player_is_the_leader_before_a_slow_friend_and_a_slow_friend_before_none() {
    let mut sim = Sim::new(zombie_flocking, 30, 12);
    let a = sim.zombie(1, 0, at(5.0, 5.0));
    let b = sim.zombie(2, 0, at(8.0, 5.0));
    let c = sim.zombie(3, 0, at(5.0, 8.0));
    // `b` may lead (`Traits[0] > 0.9`), `c` too but ranks after it.
    sim.zeds[a].bb.traits[0] = 0.2;
    sim.zeds[b].bb.traits[0] = 0.95;
    sim.zeds[b].bb.traits[1] = 0.1;
    sim.zeds[c].bb.traits[0] = 0.95;
    sim.zeds[c].bb.traits[1] = 0.5;
    sim.run(0.5);
    assert_eq!(sim.zeds[a].bb.leader_entity(), Some(id(2)), "{}", sim.path(a));
    assert_eq!(sim.zeds[c].bb.leader_entity(), Some(id(2)));
    assert!(sim.path(a).contains("LeaderAlive"), "{}", sim.path(a));
    // The player comes near: every one takes it, once its 5 s with the old
    // leader are over (a running `LeaderAlive` does not look again).
    sim.player = Some((at(6.0, 6.0), Vec2::ZERO));
    sim.run(6.0);
    for z in &sim.zeds {
        assert_eq!(z.bb.leader_entity(), Some(id(PLAYER)));
    }
}

#[test]
fn with_no_leader_and_no_enemy_it_wanders_at_a_quarter_speed() {
    let mut sim = Sim::new(zombie_flocking, 30, 12);
    let a = sim.zombie(1, 1, at(10.0, 6.0));
    sim.zeds[a].bb.traits[0] = 0.0;
    sim.run(3.0);
    assert_eq!(sim.path(a), "ZombieFlocking (select) > Flocking (action)");
    let longest = sim.zeds[a].orders.iter().filter_map(|o| o.move_dir).map(Vec2::length).fold(0.0, f32::max);
    // Its only pulls are the walls' (none near) and none to a target: the
    // stick, when written, is a quarter long.
    assert!(longest <= 0.25 + 1e-4, "{longest}");
}

#[test]
fn an_enemy_in_reach_is_flocked_at_full_speed_where_it_was_last_seen() {
    let mut sim = Sim::new(zombie_flocking, 40, 12);
    let a = sim.zombie(1, 1, at(4.0, 6.0));
    sim.zeds[a].bb.traits[0] = 0.0;
    // Too far for a super attack (16 on either axis), but within 12 to see.
    sim.enemies = vec![enemy(PLAYER, at(15.0, 6.0))];
    sim.run(2.0);
    assert_eq!(sim.zeds[a].bb.target(), Some(id(PLAYER)));
    assert!(sim.path(a).contains("WhileTargetAlive") || sim.path(a).contains("Flocking"), "{}", sim.path(a));
    assert!(sim.zeds[a].pos.x > at(8.0, 0.0).x, "it closed in: {}", sim.zeds[a].pos);
    let stick = sim.zeds[a].orders.iter().rev().find_map(|o| o.move_dir).unwrap();
    assert!((stick.length() - 1.0).abs() < 1e-4 && stick.x > 0.9, "{stick}");
}

#[test]
fn an_enemy_never_seen_is_not_flocked_at_the_origin() {
    // A room cut in two by a wall from top to bottom (the ring's own cells do
    // not block sight).
    let (w, h, cut) = (40, 12, 10);
    let mut rows = String::from("\n");
    for y in 0..h + 2 {
        for x in 0..w + 2 {
            rows.push(if x == 0 || y == 0 || x == w + 1 || y == h + 1 || x == cut { '*' } else { '.' });
        }
        rows.push('\n');
    }
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    let mut sim = Sim::new(zombie_flocking, w, h);
    sim.grid = MapGrid::build(&MapData::parse(&xml).unwrap(), None);
    sim.terrain = Arc::new(Terrain::from_grid(&sim.grid, 1.0));
    let a = sim.zombie(1, 1, at(8.0, 6.0));
    sim.zeds[a].bb.traits[0] = 0.0;
    // Within the scan distance but behind the wall: found, never in sight.
    sim.enemies = vec![enemy(PLAYER, at(13.0, 6.0))];
    sim.run(2.0);
    let bb = &sim.zeds[a].bb;
    assert_eq!(bb.target(), Some(id(PLAYER)), "found");
    assert_eq!(bb.last_known_target_position, None, "never seen");
    assert!(!sim.path(a).contains("WhileTargetAlive"), "no attack on nothing: {}", sim.path(a));
    // Not carried off to the map's top-left corner (the origin).
    assert!(sim.zeds[a].pos.distance(at(8.0, 6.0)) < 4.0, "{}", sim.zeds[a].pos);
}

#[test]
fn alone_near_a_target_in_sight_with_its_wave_ready_it_stands_two_seconds_then_fires() {
    let mut sim = Sim::new(zombie_flocking, 40, 12);
    let a = sim.zombie(1, 1, at(4.0, 6.0));
    sim.enemies = vec![enemy(PLAYER, at(12.0, 6.0))];
    // First it finds its target (the wave not ready yet).
    sim.run(1.0);
    assert_eq!(sim.zeds[a].bb.target(), Some(id(PLAYER)));
    let before = sim.zeds[a].orders.len();
    sim.zeds[a].ready = true;
    sim.run(1.0);
    assert!(sim.path(a).contains("Power"), "{}", sim.path(a));
    let standing = sim.zeds[a].pos;
    sim.run(0.9);
    assert_eq!(sim.zeds[a].pulls, 0, "not before two seconds");
    sim.run(0.5);
    assert_eq!(sim.zeds[a].pulls, 1, "{}", sim.path(a));
    assert!(sim.zeds[a].pos.distance(standing) < 0.01, "it stood");
    // `Power` held the stick at zero, with the streaks about one step in five.
    let power = sim.zeds[a].orders[before..].iter().filter(|o| o.move_dir == Some(Vec2::ZERO)).count();
    assert!(power > 100, "{power}");
    let streaks = sim.zeds[a].bb.take_effects();
    let rate = streaks.len() as f32 / power as f32;
    assert!((0.1..0.3).contains(&rate), "{rate}");
    for Effect::Teleport { at, direction } in streaks {
        let off = at - standing;
        assert!(off.x.abs() <= 2.0 && off.y.abs() <= 2.0, "within two radii: {off}");
        // Flying in, towards the zombie.
        assert!(Vec2::from_angle(direction).dot(-off) >= 0.0);
    }
}

#[test]
fn a_friend_near_holds_the_super_attack_back() {
    let mut sim = Sim::new(zombie_flocking, 40, 12);
    let a = sim.zombie(1, 1, at(4.0, 6.0));
    sim.zombie(2, 1, at(4.0, 9.0));
    sim.enemies = vec![enemy(PLAYER, at(12.0, 6.0))];
    sim.zeds[a].ready = true;
    sim.run(3.0);
    assert_eq!(sim.zeds[a].pulls, 0);
    assert!(!sim.zeds[a].orders.iter().any(|o| o.move_dir == Some(Vec2::ZERO)), "{}", sim.path(a));
}

#[test]
fn hurt_with_nobody_to_fight_it_tells_its_idle_friends_who_did_it() {
    let mut sim = Sim::new(zombie_flocking, 60, 12);
    let a = sim.zombie(1, 1, at(4.0, 6.0));
    let b = sim.zombie(2, 1, at(20.0, 6.0));
    let far = sim.zombie(3, 1, at(40.0, 6.0));
    sim.run(0.5);
    // Hit by a car out of its sight and reach.
    let sniper = enemy(70, at(58.0, 1.0));
    sim.enemies = vec![sniper];
    sim.zeds[a].bb.hurt_by(id(70));
    sim.zeds[a].health -= 4;
    sim.run(0.3);
    // `b` (16 off, within 24) took the damager; then it is its target
    // (`FindNearestEnemy` takes the damager first, whatever the distance).
    assert_eq!(sim.zeds[b].bb.damager_entity(), Some(id(70)), "{}", sim.path(b));
    assert_eq!(sim.zeds[far].bb.damager_entity(), None);
    sim.run(0.3);
    assert_eq!(sim.zeds[b].bb.target(), Some(id(70)), "{}", sim.path(b));
}

#[test]
fn the_floor_plan_says_which_way_the_room_grows() {
    let grid = room(20, 10);
    let terrain = Terrain::from_grid(&grid, 1.0);
    // Near the top wall the room grows downwards (y up: negative y), near
    // the left one to the right.
    let g = terrain.gradient_at(at(10.0, 1.0));
    assert!(g.y < 0.0 && g.x.abs() < 1e-4, "{g}");
    let g = terrain.gradient_at(at(2.0, 5.0));
    assert!(g.x > 0.0, "{g}");
    // A prediction in the wall goes to the nearest of the roomiest points.
    let p = terrain.nearest_free_point(at(0.0, 5.0), 16);
    assert!(terrain.is_point_free(p), "{p}");
    assert_eq!(p, *terrain.free_points().iter().take(16).min_by(|a, b| a.distance(at(0.0, 5.0)).total_cmp(&b.distance(at(0.0, 5.0)))).unwrap());
}
